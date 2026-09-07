# Iyon: DOM-like retained runtime

## Architecture decision and implementation handoff

**Revision:** 1, 2026-09-07  
**Repository baseline:** `alexykn/iyon-tui@1e935406c707ad42eb819d259f0a456f0a1129e7`  
**Deliverable status:** recommended implementation plan, not an implementation or a measured performance result.  
**Supersedes:** the earlier handoff that retained custom immutable `View` authoring/reconciliation as the destination.

**Read this as the implementation contract after the owner approves the changes listed in §24.** Decisions below are selected defaults, not alternatives for an implementation agent to choose between. Stop gates require evidence; they do not authorize quietly changing the architecture.

### Authoritative inputs

Both supplied documents were read in full:

| Input | Role | SHA-256 of supplied bytes |
|---|---|---|
| `DOM-LIKE-RETAINED-RUNTIME-PLANNING(1).md` | Newer simplification request, ten decisions, six required traces, migration and findings requirements | `90e134e686054e18d03e82ac5763357dd3b9a021e9f2591bf3efd6d248d822b9` |
| `IYON-UI-PRELIMINARY-DESIGN-v5(2).md` | Destination constraints and three-plane/content/React/layout contracts | `584dab63913c097042a6f36054b959a0a057989bebe3da2cdf01e48e0660889f` |

The uploaded V5 file has Git blob SHA `10a4d2d2562e872fc5eb67ffff3761480e05a912`, matching the file listed in the pinned repository tree. The current `main` was checked through GitHub before planning and again before delivery; it remained the baseline above. The atlas describes historical commit `4355c02`, not a second current implementation. Current source and the maintained architecture guide qualify its descriptions.

References use **[V5 §n]**, **[DOM §n]**, **[Rnn]** for repository evidence, and **[Xnn]** for external primary-source research. The evidence register is in §26. A statement labelled **Decision** is this handoff's recommendation, not something already implemented or approved in an input document. This distinction matters for the preliminary API changes.

## Contents

1. [Recommendation and scope](#1-recommendation-and-scope)
2. [Alternatives and research conclusions](#2-alternatives-and-research-conclusions)
3. [Closed decision register](#3-closed-decision-register)
4. [Ownership, identities and lifetimes](#4-ownership-identities-and-lifetimes)
5. [Target folders and dependency direction](#5-target-folders-and-dependency-direction)
6. [Occurrence document and topology algorithms](#6-occurrence-document-and-topology-algorithms)
7. [Properties, overrides and generated schemas](#7-properties-overrides-and-generated-schemas)
8. [UI transport and acknowledgement](#8-ui-transport-and-acknowledgement)
9. [Atomic desired-state acceptance](#9-atomic-desired-state-acceptance)
10. [React renderer](#10-react-renderer)
11. [Content integration](#11-content-integration)
12. [Native controls and events](#12-native-controls-and-events)
13. [Frame capture, receipts and failures](#13-frame-capture-receipts-and-failures)
14. [Scheduling and safe concurrency](#14-scheduling-and-safe-concurrency)
15. [Invalidation and caches](#15-invalidation-and-caches)
16. [Current-renderer integration and its deletion](#16-current-renderer-integration-and-its-deletion)
17. [Taffy and the final rendering boundary](#17-taffy-and-the-final-rendering-boundary)
18. [Surface, History and GPUI integration](#18-surface-history-and-gpui-integration)
19. [Six end-to-end traces](#19-six-end-to-end-traces)
20. [Keep, simplify, replace and delete map](#20-keep-simplify-replace-and-delete-map)
21. [Implementation tranches](#21-implementation-tranches)
22. [Verification and performance gates](#22-verification-and-performance-gates)
23. [Findings disposition](#23-findings-disposition)
24. [Owner approvals and compatibility](#24-owner-approvals-and-compatibility)
25. [Requirements closure and completion checklist](#25-requirements-closure-and-completion-checklist)
26. [Evidence register](#26-evidence-register)

---

## 1. Recommendation and scope

### 1.1 The selected design

**Use React mutation mode to issue typed changes directly to a Rust-owned mutable occurrence document. Store ordinary declared properties and explicit overrides on each occurrence. Keep Source payloads and Connector execution in the content plane.**

The bridge sends changes when semantic work is accepted. It is not a frame protocol. A native frame, scroll adjustment, input edit or delivery tick does not serialize the document or ask React to describe it again.

```text
APPLICATION / TYPESCRIPT                         NATIVE / RUST

React components
    |
React Fiber: lifecycle and reconciliation
    |
small HostInstance records
(last accepted values + native correspondence)
    |
    +-- topology/attachment operations --+
    +-- individual property deltas ------+--> OccurrenceDocument
    +-- subscription-presence changes ---+    stable identity, ordered parentage,
                                             declarations, overrides, bindings
                                                      |
                                                      v
                                             derived native execution
                                             style, layout, input, frame products
                                                      |
                                             exact presentation receipt

Source.append / replace / annotations
    |
existing separate content data transport ---> Source storage
                                                 |
                                           Connector execution
                                        configured by immutable Funnel
                                                 |
                                             ContentPort
                                                 |
                                         native measure / paint
```

At the final layout checkpoint:

```text
React -> typed deltas -> occurrences -> Taffy + content realization -> presenter
```

There is **no** production path of this form:

```text
React -> immutable Iyon View DAG -> Iyon tree reconciler -> native occurrence tree
```

React already supplies the reconciliation. Iyon supplies semantic classification, native acceptance and execution. [V5 §§0, 6, 8–10, 20; DOM §§3, 5]

### 1.2 What actually becomes simpler

The current source retains execution scopes and semantic operation slots in TS, native-reference correspondence and lease machinery in structural transport, and another native immutable-View publication runtime. The host subsequently resolves native components and prepares physical products. These have different responsibilities, but several exist specifically to bridge immutable UI values across the language boundary. [R02–R07]

Delete the following obligations rather than reimplementing them:

- Custom TS component scheduling, keyed scope matching and child-scope publication slots.
- Immutable UI-value NodeIds, native-reference promotion, weak native hints and semantic reconstruction paths.
- Root/temporary View leases, remote builders, path interning and multi-call structural edit transactions.
- Independently allocated ordinary ViewState records and rediscovery of their owning occurrence.

Do **not** delete useful native editor/scroll mechanics, immutable Source page sharing, Connector parser caches, content projection tickets, or presentation receipts merely because they are retained.

### 1.3 Two explicit implementation checkpoints

**M1 — direct-occurrence cutover.** React is the canonical frontend. The old TS composition and cross-language View publication systems are removed. The native occurrence document is authoritative. The current terminal renderer is temporarily reached through one private, one-way Rust adapter. This is a usable migration checkpoint, not the final architecture.

**M2 — renderer-adapter deletion.** Terminal general layout reads occurrences through the selected Taffy adapter; text realization consumes content semantics rather than routing ordinary UI through legacy Views. Delete the temporary occurrence-to-View adapter and redundant general-layout machinery. This is the completion point for the runtime/layout simplification described here.

The component-only Surface migration and GPUI host remain separately reviewable V5 work. Their ownership and integration contracts are fixed in §18. They must not force a new UI transport or resurrect View authoring. Full GPUI windowing, accessibility, new Source families and parser redesign are not disguised prerequisites of M1.

**Do not stop indefinitely at M1.** The implementation plan schedules M2 immediately after M1's regression gate, before adding unrelated framework features. A surviving adapter requires an explicit tracked consumer and the removal gate in §16, not “future compatibility.”

### 1.4 What this document does and does not establish

The selected architecture minimizes publication owners and repeated semantic translation under the supplied constraints. It is not a mathematical proof of global optimality or an experimentally established speedup.

The hard efficiency guarantees are narrower and executable: no-op commits transmit nothing; a one-property change does not resend sibling properties; Source appends bypass React and UI transport; native clocks send no TS-to-Rust UI work. Native layout can legitimately visit ancestors or reposition many siblings. Terminal output can still require a full viewport diff. These are measured separately.

No code, dependency, repository file, branch, or public API was changed during this planning task.

---

## 2. Alternatives and research conclusions

### 2.1 Alternatives compared

| Design | Publication owners retained | Migration cost | Deletion opportunity | Decision |
|---|---|---|---|---|
| Trim current immutable View machinery | TS scopes, immutable values, correspondence/leases, native values, host publication | Lowest immediate disruption | Some helpers; principal ownership chain remains | Reject as destination |
| Add an occurrence backend beneath existing View authoring | Old composition plus a newly written View-to-occurrence matcher | Medium now; another frontend removal later | Native leases can disappear, but another reconciler must be maintained temporarily | Reject as the default path |
| React mutation renderer + occurrence document | Fiber lifecycle, accepted HostInstance records, one native document | Explicit frontend port; backend can be staged | Removes custom composition and immutable publication together | **Select** |
| React persistence mode + immutable native shadow tree | Fiber plus native immutable versions and mount diff | Useful when snapshot/thread architecture requires it | Does not meet the simplification objective as directly | Reject for this runtime |
| Full React/Taffy/Surface/GPUI/content rewrite in one commit | Final owners can be clean | Too many unrelated regressions at once | Large eventual deletion, poor fault isolation | Reject migration strategy |

**The transition is not the destination.** A temporary adapter may derive a renderer input from occurrences. It may not become another authority or justify retaining the old public View facade.

### 2.2 Transferable primary-source lessons

**React.** Mutation mode supplies persistent host instances and insert/remove/update callbacks; persistence mode instead copies immutable host trees. Host creation can be abandoned during render, and a subtree removal need not produce one callback per descendant. These facts determine the JS-only candidate and native subtree-retirement rules below. The renderer API is experimental, so isolate and pin it. [X01, X02]

**DOM.** Borrow stable nodes, single parentage, ordered children and explicit moves. Do not import browser selectors, HTML/CSS compatibility, text-node payloads or a claim that DOM operations provide atomic rollback. Our acceptance semantics are an Iyon contract. [X03]

**Flutter.** Composition identity and derived render/layout structures can be distinct without being rival authorities. Retain dependency-based layout and native control behavior; do not copy Flutter's reconciliation under React. [X04]

**Taffy.** The high-level tree manages layout-node storage, cache and algorithm dispatch. The low-level interface can reuse another framework's tree, but requires the framework to own more integration machinery. Choose the high-level tree first because maintenance simplicity is the priority. Its parent/child representation is a disposable derived cache, not a second semantic source. [X05]

**GPUiX.** Its documented direct mutation bridge demonstrates the useful React-to-retained-native shape and JS-closure/native-listener split. Iyon must not copy its generic style/text transport as its own plane model, nor treat GPUiX's timing claims as Iyon measurements. [X06]

**Node-API and Rust.** A typed-array pointer has a call/lifetime contract; it does not authorize retaining JS memory in frames. A mutex does not make non-Send values transferable, and owner-thread checks do not justify a shared-Arc-to-exclusive-static-reference cast. The new boundary uses qualified objects, owned decoded commands and sound guards. [X07, X08]

### 2.3 Decisions intentionally not taken

Do not add a custom LIS/keyed-list reconciler beneath React. React's chosen placement callbacks determine the operations. Some reorder patterns can produce more moves than a minimal edit script. The guarantee is stable occurrences and delta operations, not a claim that every abstract list move is encoded with the fewest theoretically possible mutations.

Do not introduce a universal scene snapshot, append-only UI event log, MVCC database, root-lease manager or generic transaction manager. One short-lived prepared UI commit and one in-flight physical frame are enough for the specified ownership model.

Do not chase zero copies by retaining borrowed JS buffers or unsafe native aliases. A copy of changed ingress bytes and immutable resource sharing are acceptable. Repeated copying of the unchanged tree or accumulated document is not.

---

## 3. Closed decision register

| ID | Selected decision |
|---|---|
| D01 | React/Fiber is the only general frontend reconciler. |
| D02 | One host-owned occurrence document owns topology and ordinary declared/override state. |
| D03 | Occurrences have one ownership parent; resource sharing uses distinct occurrences. |
| D04 | React render creates only JS candidates. Native resources are prepared during commit. |
| D05 | Use a small generational arena and intrusive sibling links for native topology. |
| D06 | Detached occurrences may exist inside a commit; no public long-lived detached-node ownership API is added. Hidden React subtrees remain mounted. |
| D07 | Ordinary ViewState handles disappear. Restricted occurrence refs perform override operations. |
| D08 | Structure/state/binding/subscriptions share one typed UI commit envelope, retaining their semantic classification and counters. |
| D09 | Public Source payload mutation stays on the existing same-image content data lane. |
| D10 | React-owned literal replacements occupy a separate content section of the UI envelope and use the same content validation/storage implementation. No N-API Source.append alternative is added. |
| D11 | Use one generated N-API UI batch entrypoint, not scalar/fixed-arity/path/FFI UI alternatives. |
| D12 | Preflight through a sparse overlay, reserve storage, then apply an infallible prepared commit. No mutate-then-undo strategy for external effects. |
| D13 | Accepted UI revision, native work epoch, Source revision and physical frame ID are distinct. |
| D14 | A frame owns exact immutable products/pins. It never borrows mutable occurrences after capture. |
| D15 | Permit only one prepared/submitted physical frame per host; newer desired work coalesces. |
| D16 | Retire handles immediately; reclaim payloads according to actual owned frame/resource references. |
| D17 | Reuse the environment pending-host queue; native scheduling, not a TS frame loop, drives deadlines and receipts. |
| D18 | Consolidate property validation/effects in the existing generator. No generic props map or complete Style wire format. |
| D19 | One private legacy renderer adapter is permitted at M1; no legacy TS frontend adapter. Delete it at M2. |
| D20 | TaffyTree is the initial terminal layout integration. GPUI uses its own Taffy-backed frame integration, not a second layout pass over those same elements. |
| D21 | Preserve existing content parsers/storage/delivery during the occurrence cutover; their independent V5 refinements stay explicit. |
| D22 | Keep static text, Markdown, ANSI, diff and streaming under the common content boundary. |
| D23 | Surface migration preserves resident mutability; semantic completion is not a rendering lifetime. |
| D24 | Unknown partial scrollback writes prohibit automatic replay. Logical rollback cannot repair external terminal history. |
| D25 | No permanent old/new production selector after a checkpoint's cutover. Baseline comparisons use separate builds. |

---

## 4. Ownership, identities and lifetimes

| Entity | Owner | State kind | Update trigger | Release condition |
|---|---|---|---|---|
| Fiber | React root | Frontend lifecycle | React work | Unmount or abandoned work |
| HostInstance | React renderer | Accepted correspondence and normalized values | Successful UI acceptance | Unmount; abandoned candidates are JS-only |
| Occurrence | Host document | Authoritative native UI state | Accepted typed UI operations | Logical retirement invalidates key immediately |
| Explicit override | Occurrence | Authoritative override layer | Ref operation | Clear or occurrence retirement |
| Editor/scroll/animation state | Native control owner | Authoritative execution state | Native input, command or deadline | Implicit owner retires, or explicit owner disposes |
| Source | Environment registry plus resource owner | Authoritative content/lineage/revision | Accepted content operation | Explicit/implicit owner disposal; immutable snapshots may outlive the handle |
| Funnel | Immutable value/resource owner | Configuration | New specification | Last value/native descriptor reference released |
| Connector | Host content registry and resource owner | Binding-local execution | Source/demand/deadline/capability changes | Unselected/unused disposal, after required product pins |
| ContentPort | Host content registry and resource owner | Destination and requested/selected binding | Attachment/control/geometry | Unmounted and unused; old products retain their own data |
| Layout node/cache | Renderer driver | Derived | Dirty geometry/topology/content metrics | Eviction, occurrence removal or driver close |
| Prepared projection | Candidate/cache | Immutable derived product | Content preparation | Last exact ticket released |
| Physical frame | Presenter | Captured output and hit/cursor metadata | Native preparation | Submission failure or receipt completion and replacement |
| Confirmed viewport metadata | Presenter | Last confirmed physical state | Exact successful receipt | Replaced by later confirmed frame or close |
| Scrollback export ledger | Terminal backend | Confirmed irreversible progress | Sink acknowledgement | Session close; never reset by a React commit |

### 4.1 Identity representation

Use a host-local native key:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
struct NodeKey {
    slot: u32,
    generation: u32,
}
```

The UI wire handle is four u32 words: `(host_namespace, slot, generation, resource_kind)`. Zero is invalid in each existing-handle field. The kind distinguishes nodes, Ports, Connectors and controls even when their arena indices coincide. `host_namespace` is a checked process-unique nonrecycled namespace; the qualified N-API host object additionally establishes environment/addon ownership. Do not pass a native pointer or pack a u64 into an imprecise JS Number.

Slot generation starts at 1 and increments before reuse. At `u32::MAX`, burn the slot instead of wrapping. Namespace exhaustion is an explicit creation error. These integer fields are private wire representation, not a public application API exposing arena indices.

Source identities retain their existing environment/source generation qualification. Do not renumber all resource families merely to make the diagrams look uniform.

### 4.2 Sharing rules

One reusable React element value rendered in two locations produces two mounted occurrences. One Source may feed two independently paced/width-specific Connectors. Immutable styles, semantic blocks and Source pages may be shared.

One mutable occurrence, exclusive native controller, or ContentPort cannot be mounted in two desired locations. Validate the final desired binding map, not the union of the old physical frame and new desired tree. An old frame's pin is not a second desired mount.

### 4.3 Detachment decision

`DETACH` is an internal transaction operation, primarily for safe ordered moves. By acceptance, every surviving node must belong to a host-owned root or a controller-owned root. Reject a surviving orphan. No detached occurrence lease, keepalive handle or browser-style external node ownership API is introduced.

A normal React removal emits retirement. A React hidden subtree keeps its ownership edges and resources but loses render/input demand. An explicitly owned Port or controller can remain unmounted and later mount in a **new** occurrence; that independent resource lifetime does not keep an old occurrence alive.

### 4.4 Roots and ownership edges

The host has an internal body root and, during migration, typed History roots. Native animation frame roots or controller-body roots are owned beneath their controller occurrence. Root roles affect layout participation; they do not remove the ownership edge from cycle validation.

Thus an animation cannot be inserted inside its own inactive frame. Cycle checks follow ownership parents, including internal root containers. Root containers are internal identities, not an arbitrary public Root host kind. Same-host portals target these qualified containers. Cross-host portals are rejected; use another React root.

### 4.5 Retirement versus storage reclamation

Retirement removes desired edges, callbacks, input eligibility and future ticking immediately. The old handle never becomes valid again.

An already submitted frame retains owned output and exact product/resource pins, not a live-node lookup. Therefore a node slot may be reclaimed/reused once its record is no longer needed by the capture implementation; generation checks prevent stale input from addressing its replacement. When an old adapter still requires a pinned payload, move that payload into retirement ownership rather than making the retired node mutable or interactive.

Never invoke application callbacks or thread-affine destructors while applying retirement under the document guard. Resource releases that require an owner thread are queued to that owner with owned data. Reclamation is local to this host; one host closing cannot abort another host's work.

---

## 5. Target folders and dependency direction

Keep the existing workspace/package names for this task. A rename to `iyon-ui` is not part of fixing runtime ownership. Paths marked **new** are proposed; existing responsibilities are identified in §20.

```text
packages/iyon-tui/src/
  react/                              new
    index.ts                          public React entrypoint and root creation
    host-config.ts                    pinned reconciler adaptation only
    instance.ts                       candidates, accepted snapshots, refs
    commit.ts                         one commit journal and acceptance coordinator
    components.tsx                    Box/Row/Column/Content/Text/control conveniences
    content-hooks.ts                  lazy commit-owned Port/Connector tokens
  transport/
    ui/                               new; replaces structural/ and ordinary state sessions
      session.ts                      one qualified N-API commit adapter
      generated/                      generated IDs, encoders, property descriptors
    content/
      transport.ts                    typed ContentDataTransport seam
      ffi.ts                          existing sole public Source-data implementation
    native/                           addon loading and resource qualification retained
  api/content/                        existing Source/Funnel values and typed factories
  runtime/                            host lifecycle, errors and event delivery; no frame clock
  testing/                            existing testing entrypoint, adapted to React roots

crates/iyon-tui/src/
  occurrence/                         new, private implementation
    mod.rs                            document facade and record types
    arena.rs                          generation allocation/validation/reclamation
    tree.rs                           ownership links, insert/detach/retire
    properties.rs                     declared/override/effective state
    commit.rs                         prepared changes, validation, reserved apply
  application/
    host.rs                           authoritative host/document/control integration
    environment.rs                    existing shared Source/wake owner, native scheduling
    frame.rs                          new: capture/presentation state and receipt promotion
    legacy_scene.rs                   new TEMPORARY: one-way current-renderer adapter
    kernel.rs                         native control/input execution; no new authoring API
    content.rs                        existing Connector/Port integration
    source_store.rs                   existing persistent content storage
  presentation/
    taffy.rs                          new at M2: terminal general-layout adapter
    ...                               retain paint/physical helpers that still have callers
  content/text/                       keep semantic types, projectors and content policy
    terminal.rs                       new at M2: direct semantic-content/Taffy projection
  controls/                           keep concrete native editing/scroll behavior
  history/native/                     keep confirmed-prefix/physical transfer mechanics
  theme/, physical/, terminal/        retain actual semantic/physical host functions

crates/iyon-tui-native/src/
  tui/ui_commit.rs                    new generated-entrypoint glue; guarded host access
  generated/ui_*.rs                   new generator output
  content_ffi.rs                      existing content data lane, same qualified image
  tui.rs                             lifecycle, Sources, diagnostics and events

tools/tui-abi/
  ui_abi.toml                        new finite UI opcode/value/property schema
  content_abi.toml                   new description of EXISTING content packing
  view_abi.toml                      remove superseded entries/output after cutover

tools/tui-abi-gen/                    extend the existing generator, not a new framework
```

`content-hooks.ts` is only a lifecycle/token helper. It is not a scheduler or second resource registry with independent native authority. Small helpers can stay in their owning module; this tree is not a mandate to split each operation into a separate file.

### Dependency rules

```text
React adapter -> UI session -> generated decoder -> occurrence commit/document
                                               -> existing content prepare/install
                                               -> native control prepare/install

renderer driver -> read/capture of document + content + controls
legacy adapter  -> private old renderer factories (temporary, one-way)
Taffy adapter   -> Iyon geometry semantics, content leaf measure, Taffy
presenter      -> owned output/receipt, never mutable JS or live semantic lookup
```

The occurrence modules do not import React, N-API, Termwiz, GPUI or Taffy. Content parsers do not import UI topology. A backend can discard its entire derived layout cache without changing authoritative UI semantics.

Keep layout/backend objects in the renderer driver's ownership domain. Do not assume a particular Taffy feature set or GPUI platform object is Send. The portable semantic core and cross-thread messages must satisfy compiler-checked Send requirements; thread-affine drivers are constructed and used on their owner thread.

---

## 6. Occurrence document and topology algorithms

### 6.1 Minimal native record

```rust
struct Links {
    parent: Option<NodeKey>,
    first_child: Option<NodeKey>,
    last_child: Option<NodeKey>,
    previous_sibling: Option<NodeKey>,
    next_sibling: Option<NodeKey>,
    child_count: u32,
}

struct Occurrence {
    kind: HostKind,
    links: Links,
    declared: DeclaredProperties,
    overrides: Overrides,
    attachment: Attachment,
    subscriptions: EventMask,
    renderer_hidden: bool,
    revisions: Revisions,
    dirty: DirtyStamp,
}
```

Use a `Vec<Slot>` plus free-slot stack, not one mutex/Arc per node. Fixed link records are cheap to copy into a transaction overlay. Variable-size immutable values may use Arc-backed strings/slices; do not clone accumulated text or a complete subtree for a field edit.

The public behavior families and version-one HostKind codes are `Box=1`, `ContentHost=2`, `Editor=3`, `Scroll=4`, and `Animation=5`. ControlKind codes are `Editor=1`, `Scroll=2`, and `Animation=3`. Internal roots are created only by CREATE_ROOT, not a public HostKind. Internal root containers have an internal role. Row/Column/Grid/Spacer/decorators are conveniences over Box state. `Scroll` becomes the V5 ScrollSurface's occurrence behavior when its spatial API migrates. No Text, Diff, Markdown or Hanging kind is added to the structural vocabulary.

An indexed child vector is a **derived** cache, generated by following sibling links when a backend needs indexing. Link mutation is bounded; materializing that vector costs O(parent degree). Do not conflate the two costs.

### 6.2 Why sibling links

They make known-node insertion/removal/move direct, avoid cloning a wide parent's authoritative child vector during validation, and require only a few auditable operations. The tradeoff is less cache-friendly traversal and a derived vector where Taffy needs one. This is a selected representation, not an assertion that linked storage always benchmarks faster than Vec children.

Only `tree.rs` may edit link fields. Downstream code relies on its invariants instead of repeating cycle/duplicate-parent checks in layout and paint.

### 6.3 `insert_before(parent, child, before)`

All reads/writes below use the transaction overlay, not live authoritative records:

```text
validate parent, child, and allowed root/kind relationships
if before exists: require before.parent == parent
reject movement of a protected root via this ordinary operation
if before == child: return unchanged
if child.parent == parent and child.next_sibling == before: return unchanged

walk ownership parents starting at parent:
    if an ancestor is child: reject CYCLE

unlink child from its old parent if present
previous = before.previous_sibling if before exists else parent.last_child
set child.parent = parent
set child.previous_sibling = previous
set child.next_sibling = before
if previous exists: previous.next_sibling = child
else: parent.first_child = child
if before exists: before.previous_sibling = child
else: parent.last_child = child
increment parent.child_count, checked
```

Validate `before` **before** treating `before == child` as a no-op: a child from a different parent is not a valid anchor. Count arithmetic is checked before publication. Ancestor checking is O(height); link edits touch at most seven distinct node records. No descendants are recreated.

Sequential topology semantics are intentional. A batch that temporarily introduces a cycle is invalid even when a later command would break it. Detach the relevant edge first. There is no second whole-graph reconciliation to reinterpret command ordering.

### 6.4 `unlink` and `detach`

Capture old parent/previous/next before modifying anything. Repair both neighbor links and the parent's first/last/count. Clear the child's parent/previous/next. Do not touch descendants, control state or resource generations.

`detach(parent, child)` requires that exact overlay edge. A wrong-parent detach is an error, not successful cleanup. The final-orphan check ensures that a detached survivor is reattached before acceptance.

Because demand changes are computed from final desired ownership, a same-commit move does not temporarily dispose the Port, clear focus or restart the editor.

### 6.5 `retire_subtree(root)`

Enumerate iteratively through final overlay children. A descendant moved out earlier in the batch survives. Unlink the root and prepare retirement for the enumerated nodes, subscriptions and implicit resources.

React can report only the removed top-level host child. Native retirement and JS callback cleanup must therefore cover all descendants. Do not wait for separate removal callbacks or a later GC sweep.

At apply, invalidate the keys and remove desired resource ownership. Detach implicit bindings, retire implicit Connectors, release their Source memberships, and release the implicit Port. Externally owned resources merely become unmounted/unused. Their caller still owns disposal.

### 6.6 Final validation

Check only the affected frontier plus required ancestor/subtree traversals:

- Every surviving created/detached/moved root reaches a legal ownership root.
- Every affected parent satisfies its kind's child rules.
- Every affected Port/control has at most one final desired owner.
- No surviving attachment references a resource being disposed.
- The roots/animation-frame ownership graph is acyclic, including internal containers.

A single property patch must not scan every node to rediscover all attachments. Maintain direct resource-owner indices and update their touched entries in the transaction plan.

### 6.7 Resource lifetime and generation tests

Cover create/move/retire; anchor self/no-op; wrong anchor parent; cycle; descendant rescue before parent retirement; failed batch after valid earlier edits; same numeric slot reused with a different generation; and cross-host/cross-kind handle substitution.

A prepared old frame may still mention the old key. Its owned data remains usable, but new input/mutation through that key is rejected. No lookup is allowed to reinterpret it as the new generation.

---

### 6.8 Kind cardinality and root roles

`Box` accepts zero or more ordinary children. `ContentHost` and `Editor` accept none; their payload is content. `Scroll` accepts ordered children through one derived content container. `Animation` accepts an ordered set of ordinary frame-root children, normally Boxes; the control selects one child's layout/paint participation. These frame roots need no special public native kind. All children, including inactive frames, retain normal ownership edges.

Internal RootRole codes are `Body=1`, `Portal=2`, and temporary `LegacyHistoryUnit=3`. Body exists once per host and cannot be retired through an ordinary operation. Portal roots require an owning same-host node; their owner edge participates in cycle/lifetime checks. A History unit root is host-owned; its typed config uses the existing `FlowBoundary` enum and optional existing unit identity when adapting a current unit. Creation of a new unit allocates its native unit identity during resource preflight and installs it atomically with the root. The legacy adapter keeps the root-to-unit mapping; the UI wire does not publish immutable View identities.

`HISTORY_ACTION(Freeze)` validates the final same-commit replacement children against the current final-unit contract (no live native component/controller content). It changes unit semantics only; it does not freeze Source presentation. `DiscardLive` validates current tail/live restrictions and retires that root. `RETIRE_ROOT` is allowed only through its actual owner; it cannot bypass History restrictions. These three History-specific schema fields/operations are deleted at the Surface migration gate.

Ordinary `<Text>` initially accepts strings/numbers and the migrated finite styled-span convenience, not arbitrary semantic documents or nested layout children. It lowers directly to a childless ContentHost. Raw JSX text under a Box creates an ordinary content child. Do not manufacture an invisible Box just to work around leaf cardinality.

---

## 7. Properties, overrides and generated schemas

### 7.1 One semantic manifest

Extend the existing `tui-abi-gen`. The manifest supplies:

```text
PropertyId, semantic name, legal host kinds, value type,
normalizer, default/reset, override behavior, inheritance,
semantic effects, native realization requirements
```

Generate TS types/normalization/equality/encoding, Rust decode descriptors/effect metadata, and a compact schema reference. Keep handwritten execution algorithms separate from generated repetitive packing.

Fingerprint **every** semantic input, including kind codes, enums, property IDs, record layouts and content annotation formats. The historical kind-code fingerprint gap is not carried forward. Regenerate outputs, never hand-edit generated files. [R10]

### 7.2 Initial property inventory

M1 must migrate the existing supported semantics, not silently replace them with arbitrary CSS:

| Domain | Required semantics | Owner/effects |
|---|---|---|
| Geometry | Current size rules, min/max, padding, supported alignment, gap, item track sizing, grid tracks/placement/spans | Occurrence; layout input |
| Presentation | Foreground/background, border intent/glyphs/edges, attributes, named/direct styles, caller-defined style states | Occurrence; presentation, plus layout for geometry-changing border edges |
| Interaction | Focus eligibility, disabled/hidden behavior, existing control configuration | Occurrence/control; interaction effects |
| Content policy | Plain/Markdown/diff/ANSI, wrapping, links, delivery policy | Funnel/Connector, not Box properties |
| Payload | Static labels, document replacements, stream chunks, annotations | Content operations, never state/structure |

M2 extends the Iyon-owned geometry schema with the V5 Flex/Grid fields: display, flex direction/wrap/grow/shrink/basis, margins, alignment/justification, gaps, grid templates/auto-flow/placement/spans and portable positioning. Native `taffy::Style` is never an exposed value type. Unsupported M1 geometry is rejected, not silently approximated until Taffy arrives.

Use stable explicit IDs, not enum ordinal-derived IDs. Existing terminal rules are the M1 behavior oracle. The generated manifest emitted in the schema tranche becomes the exact numeric ID table; an implementation agent is not free to create independent TS/Rust tables.

### 7.3 Normalize before comparing

Normalize finite numbers, bounded integers, enums, typed lengths/colors, null/reset semantics and structured values. Reject invalid values at public normalization and again at native ingress. Do not run speculative validators repeatedly inside trusted rendering code.

Copy/freeze caller-owned arrays before retaining them as accepted truth. Compare structured fields semantically, not object identity, serialization strings or a collision-prone hash alone. A stable object whose contents were mutated cannot be allowed to rewrite the accepted snapshot in place.

Named style strings must obey the same whitespace/NUL contract in TS and Rust. Do not reproduce the historical overescaped TS regular expression.

### 7.4 Declared, override, native and effective layers

```text
React declared base
    + explicit occurrence override
    + native interaction/control facts
    + inherited theme/environment context
    = derived effective values
```

A base update never erases an explicit override. Clearing an override reveals the latest base:

```text
base blue -> override red -> base green -> clear override
visible/effective: blue -> red -> red -> green
```

The masked base update is still a desired-state change. It advances UI acceptance but need not invalidate layout/paint. §13 defines metadata-only presentation completion so its visibility barrier does not hang waiting for unnecessary output.

`setOverride`/`clearOverride` use the same host session and target the occurrence; there is no independently allocated ordinary state attachment. Preserve a standalone state/control handle only for an actual independent resource lifetime—not to address gap/background fields.

### 7.5 Coalescing and snapshots

Within a commit, last write wins per `(handle, PropertyId, layer)`. Keyed style-state entries coalesce per `(handle, state-key, layer)`. Compare the final value with the last **accepted** value. A sequence `A -> B -> A` produces no transport.

Do not sort topology operations like properties. Preserve React's structural callback ordering. A failed native acceptance must not become the next comparison baseline.

Callback closures are excluded from native property equality. Only the event-presence mask crosses; changing the function while a listener remains present is JS-only.

---

## 8. UI transport and acknowledgement

### 8.1 Canonical entrypoint

Use one qualified native host/session method:

```ts
commitUiV1(
  words: Uint32Array,
  metadata: Uint8Array,
  ownedContent: Uint8Array,
  sources: readonly QualifiedNativeSource[],
): Uint32Array;
```

`words` contains typed sections. `metadata` contains changed style names, finite vector values and Funnel descriptors. `ownedContent` contains only separately classified React-owned content replacement data. Public Source append/replace continues through `ContentDataTransport` and the existing content FFI.

`QualifiedNativeSource` is the existing native Source object, not an arbitrary props object. Unwrap and validate native type/environment before taking host mutation guards. Ports/Connectors/controls in the new UI session use typed private handles; Source data identities keep the existing content ABI representation.

One call is made per nonempty accepted frontend commit. There is no begin/commit/abort identifier that survives across JS calls. Reusable encoder buffers are local to the session and grow geometrically; avoid a global mutable scratch slot that can be overwritten by another host or reentrant operation.

### 8.2 Private wire format

All integers are little-endian u32 values; decode explicitly rather than casting arbitrary byte offsets to aligned Rust structs. The version-one header is **16 words**:

| Word | Field |
|---:|---|
| 0 | Magic `0x49595549` |
| 1 | Version `1` |
| 2 | Total word count, including header |
| 3 | Host namespace |
| 4–5 | Expected accepted UI revision, low/high |
| 6 | Total local creation count across node/Port/Connector/control kinds |
| 7 | Structural section word count |
| 8 | State/control section word count |
| 9 | Content-control section word count |
| 10 | Event section word count |
| 11 | Owned-content descriptor section word count |
| 12 | Metadata byte count |
| 13 | Owned-content byte count |
| 14 | Source reference count |
| 15 | Reserved, zero |

The five sections follow in header order. Each record is `[opcode, record_word_count, operands...]`, including its two header words. The sum of section sizes plus 16 must equal the exact input length.

Handles are four words `(host, slot, generation, kind)`. Kind codes: `1=node`, `2=Port`, `3=Connector`, `4=control`. A local reference is `(0, ordinal, 0, kind)` with ordinal in `1..creation_count`. Null is four zero words only where permitted. Creation records must define every ordinal exactly once with the referenced kind. Local references never survive acknowledgement. The canonical encoder drops never-mounted candidates. The raw v1 decoder rejects create-and-retire of a local node/root in one batch, so every returned creation mapping is live at acknowledgement; later explicit retirement is an ordinary next commit.

### 8.3 Opcode families and operands

The generator assigns the following explicit opcode values. `H` means four-word handle/reference; `S` means `(offset,length)` into metadata; `B` means `(offset,length)` into owned content. Property value words are described by the generated PropertyId descriptor.

| Opcode | Section | Operands and contract |
|---:|---|---|
| `0x01` | Structure | `CREATE_NODE(localOrdinal, HostKind)` |
| `0x02` | Structure | `CREATE_ROOT(localOrdinal, RootRole, owner:H-or-null, config:S)`; internal roles only |
| `0x03` | Structure | `INSERT_BEFORE(parent:H, child:H, before:H-or-null)` |
| `0x04` | Structure | `DETACH(parent:H, child:H)` |
| `0x05` | Structure | `RETIRE_SUBTREE(root:H)` |
| `0x06` | Structure | `ATTACH_PORT(node:H, port:H-or-null)` |
| `0x07` | Structure | `ATTACH_CONTROL(node:H, control:H-or-null)` |
| `0x08` | Structure | `CREATE_CONTROL(localOrdinal, ControlKind, ownershipMode, ownerNode:H-or-null, config:S)` |
| `0x09` | Structure | `DISPOSE_CONTROL(control:H)` |
| `0x0a` | Structure | `HISTORY_ACTION(unitRoot:H, ActionId)`; temporary Freeze=1/DiscardLive=2 only |
| `0x0b` | Structure | `RETIRE_ROOT(root:H)`; owner-qualified non-body root only |
| `0x10` | State | `SET_DECLARED(node:H, PropertyId, valueWords...)` |
| `0x11` | State | `RESET_DECLARED(node:H, PropertyId)` |
| `0x12` | State | `SET_OVERRIDE(node:H, PropertyId, valueWords...)` |
| `0x13` | State | `CLEAR_OVERRIDE(node:H, PropertyId)` |
| `0x14` | State | `SET_HIDDEN(node:H, boolean)`; renderer flag, not author display |
| `0x15` | State | `SET_STYLE_STATE(node:H, layer, key:S, typedValue...)` |
| `0x16` | State | `CLEAR_STYLE_STATE(node:H, layer, key:S)` |
| `0x18` | State | `CONTROL_COMMAND(control:H, CommandId, typedOperands...)` |
| `0x20` | Content control | `CREATE_PORT(localOrdinal, family, ownershipMode, ownerNode:H-or-null)` |
| `0x21` | Content control | `CREATE_CONNECTOR(localOrdinal, sourceIndex, port:H, funnel:S, ownershipMode)` |
| `0x22` | Content control | `SELECT_CONNECTOR(port:H, connector:H-or-null)` |
| `0x23` | Content control | `DISPOSE_CONNECTOR(connector:H)` |
| `0x24` | Content control | `DISPOSE_PORT(port:H)` |
| `0x25` | Content control | `SET_LITERAL_FUNNEL(port:H, funnel:S)`; private literal destination only |
| `0x30` | Events | `SET_SUBSCRIPTIONS(node:H, maskLow, maskHigh)` |
| `0x40` | Content descriptors | `REPLACE_LITERAL(port:H, contentFormat, data:B, annotations:B)`; private occurrence-owned block only |
| `0x41` | Content descriptors | `REPLACE_EDITOR_CONTENT(control:H, data:B, expectedEditRevisionLow, expectedEditRevisionHigh)` |

Creation descriptors across all sections are collected first during preflight, so a structural attachment may reference a Port created in the later content-control section. Structural operations themselves execute in their recorded order. Properties are final-value coalesced. Resource ownership is validated against the final result, not arbitrary section order.

A literal Port's private block Source and plain/semantic Connector are prepared internally by the content implementation. They follow the same content storage/projection contract, but no public Source handle is required for a static label. On replacement, reuse that private Source/Port identity. Source-backed `<Content>` uses explicit Connector create/select operations instead. A private literal destination uses `SET_LITERAL_FUNNEL` for changed wrap/format/delivery configuration without retransmitting unchanged literal bytes; that operation prepares a new implicit Connector against the same private Source. The default is plain/immediate. No Source payload is part of a control descriptor.

Control commands are a closed generated set matching the native controls: focus, editor selection/control actions, scroll commands, and animation playback/configuration. Text bytes never appear among their state operands; editor replacement is `0x41`. New command families require schema changes, not generic JSON bodies.

### 8.4 Value encodings and bounds

Fixed values use generated exact word counts: booleans `0/1`, u32 enums, checked signed integer lanes, finite f32 bit patterns for the portable scalar fields that allow them, and low/high pairs for u64 counters. Strings are UTF-8 slices with explicit length, not C strings. Structured arrays such as grid tracks use count plus bounded metadata slices and one generated element format.

Reject unknown opcodes/property IDs, wrong section, invalid kind, reserved bits, noncanonical booleans, nonfinite scalars, invalid UTF-8, overlapping-invalid payload formats, arithmetic overflow, trailing words, out-of-range source indices, and offset/length overflow. Do not skip unknown records as forward compatibility.

Initial admission caps: 16 MiB control words as bytes, 4 MiB metadata, and the existing 64 MiB content-mutation cap for the content section. Record and creation counts must fit those buffers and the configured host arena capacity, initially 1,048,576 live/provisional occurrences and resource slots in total. Limits may be configured lower at host creation; they are admission bounds, not eager allocations. Exceeding capacity rejects the entire batch; never split one supposedly atomic commit into separately accepted pieces behind the caller's back.

### 8.5 Acknowledgement

Return an array allocated **before apply**:

```text
status
accepted_ui_revision_low
accepted_ui_revision_high
created_count
failed_record_or_0xffffffff
detail_code
wake_flags
reserved_zero
then, in ordinal order: four words per created handle
```

Preallocate the JS ArrayBuffer/typed-array result before authoritative changes. Filling this memory and returning its already-created N-API value must not require a fallible conversion after acceptance. A convenience binding that allocates a JS return value from a Rust Vec only after mutation does not meet this contract.

On rejection, accepted revision is unchanged. Error codes distinguish malformed input, stale revision/handle, wrong host/kind, invalid topology/binding, in-use disposal, capacity failure and internal invariant fault. Diagnostic strings are formatted outside apply; fixed error codes and the offending record establish the synchronous contract.

A successful acknowledgement promotes JS snapshots and local handles. A later wake/presentation error cannot turn that acknowledgement into a rejection or cause a duplicate append/replayed commit.

### 8.6 Buffer safety

Only non-shared, attached backing ArrayBuffers are accepted for UI inputs. Check the backing object with the runtime's native array-buffer predicates before making Rust slices. A typed-array type check alone is insufficient. Qualify SharedArrayBuffer, detached-buffer and nonzero-byte-offset cases against Bun 1.4.0 in the native boundary tests; Node-API documentation is a contract reference, not proof of Bun's implementation. [X07]

Copy changed control/metadata data into an owned prepared representation before any await or native mutation. Copy private replacement bytes into the content store's prepared ownership boundary. Do not retain JS memory in a frame. Unwrapping source references/getters, or any call capable of executing JS, happens before final preflight and with no live host guard.

This intentionally chooses a safe bounded copy over a brittle universal zero-copy claim. Public large streaming payloads continue to use the existing qualified data path.

---

## 9. Atomic desired-state acceptance

### 9.1 Exactly one logical boundary

A UI batch changes one host's accepted desired UI revision. Source appends and native input/deadlines advance Source/control/work revisions, not the JS publication revision. One JS session coordinates all React and imperative override publications for a host.

A stale expected UI revision rejects unchanged. It is a producer-ordering bug on the canonical path; do not recover by serializing the entire tree again.

### 9.2 `prepare_ui_commit`

Implement in this order:

1. Decode/bound-check buffers and qualify external Source references; no host mutation.
2. Acquire the host guard; check lifecycle, namespace and expected UI revision.
3. Reserve local creation keys, owned values, plan vectors, result storage and affected-entry maps. Provisional objects are not live registry members.
4. Build a sparse overlay: unchanged reads use the document; first writes copy only touched fixed links/metadata and record property/resource changes.
5. Interpret structural operations sequentially using §6. Check cycle and root rules.
6. Apply normalized final property layers to the overlay; determine actual effective effects. A masked base change remains a base change.
7. Prepare provisional Port/controller records and collect final binding/Funnel requests. Do not yet execute Connector selection against a private Source that has not been prepared.
8. Prepare private Source/editor replacements from the content section using the existing storage/control validators. This includes a new literal destination's private Source candidate. Do not parse/layout/write to the terminal here.
9. Prepare Connectors and Source memberships using those provisional resources. Validate final attachment uniqueness, affinity, families, current generations and disposal restrictions across all sections.
10. Enumerate final retired subtrees and prepare exact cleanup actions. Validate final ownership roots and surviving references.
11. Reserve dirty queues, retirement bookkeeping, registry insertion capacity and counter increments. Return an owned `PreparedUiCommit`.

A plan is local to this call. Aborting drops provisional objects and buffers; accepted state, visible state and live resource memberships do not change. Reservation may increase spare capacity, but may not publish a usable handle or a Source revision.

### 9.3 `apply_prepared_ui_commit`

Under the same serialized ownership boundary:

```text
install reserved live records
install final links and declared/override/subscription values
install prepared resource memberships and binding requests
install private content/editor replacements
invalidate retired handles and move necessary payloads into retirement ownership
advance UI revision once for a nonempty desired change
advance/schedule the captured publication work epoch
mark effective layout/content/presentation work from the prepared effect set
fill the already allocated acknowledgement
unlock
notify native scheduling
```

No N-API conversion, renderer call, application callback, terminal I/O, fallible parser, registry lookup that can newly fail, or unreserved allocation belongs in apply. Split old setters that mutate and immediately render into prepare/install/schedule. In particular, do not simply call the current `set_view`/`render` chain inside a function named transaction. [R06, R07]

Fatal allocator aborts/panics are not recoverable transactional errors. Use checked reservation for caller-controlled sizes; an invariant failure after apply begins faults the host. Do not report “unchanged” after partial mutation.

### 9.4 Why not an undo journal

An undo journal requires inverses for controller state, memberships, retirement and content storage. It also tempts callers to include irreversible output. The sparse plan retains only the final touched state, can be discarded cheaply, and performs no external action before acceptance.

It is not a persistent immutable UI graph: only the changed records for one call are staged. No versioned full-tree root, NodeId correspondence or transaction handle survives acceptance.

### 9.5 Cross-registry synchronization

The lock order is host core → environment Source registry when needed → affected Source guards in identity order. Source data mutation releases its Source/registry guard before host wake fanout. Environment scheduling removes pending work/weak handles and releases its queue guard before acquiring a host.

Hold the relevant registry guards from final resource validation through installation. Owned provisional objects remain unpublished while those guards are held. Do not validate `SOURCE_IN_USE` and later reacquire a changed registry before installation; a competing disposal would invalidate the guarantee.

Ordinary property commits do not acquire Source registry locks. Public external Source mutation is not smuggled into this atomic UI transaction; only private occurrence-owned replacements and explicit prepared membership changes participate.

### 9.6 No-op cases

An empty frontend journal makes no native call. A nonempty wire batch that normalizes to no desired change returns the existing revision without scheduling work.

A base change masked by an override, or a native subscription-presence change, can require only publication metadata rather than paint. It still has a definite accepted revision and receives the metadata-only visibility completion in §13. No unnecessary layout is required to make the revision observable.

---

## 10. React renderer

### 10.1 Pins and supported surface

Initial proposed runtime pins are **React 19.2.8** and **react-reconciler 0.33.0**, with the reconciler types pinned at **0.33.0** and resolved React types locked in `bun.lock`. The React 19.2.8 source package declares reconciler 0.33.0, React peer `^19.2.8` and Scheduler `^0.27.0`. Inspect the installed package manifest at the first build too; source-tag metadata is not an assertion that a previously published npm artifact was republished. [X02]

Use mutation mode; no persistence, hydration, browser resources/singletons, selector-testing integration or experimental view-transition features. Use the installed HostConfig signatures, not a pre-React-19 tutorial. The shim is isolated in one file and upgraded only with its contract tests.

### 10.2 HostInstance

```ts
interface HostInstance {
  readonly root: HostRoot;
  readonly kind: HostKind;
  handle: OccurrenceHandle | undefined;
  accepted: AcceptedSnapshot | undefined;
  initial: NormalizedProps;
  initialChildren: HostInstance[];
  parent: HostInstance | RootContainer | undefined;
  firstChild: HostInstance | undefined;
  lastChild: HostInstance | undefined;
  previousSibling: HostInstance | undefined;
  nextSibling: HostInstance | undefined;
  lifecycle: "candidate" | "accepted" | "retired";
}
```

Links track frontend ownership and cleanup, not layout. AcceptedSnapshot contains structural attachment identities, declared state, requested content binding and subscription presence. Native editing, scrolling, parser progress, effective layout, frame receipts and computed style do not live here.

### 10.3 HostConfig contract

| Callback/family | Iyon behavior |
|---|---|
| `createInstance` | Pure normalization and JS candidate creation; no native resource |
| `createTextInstance` | JS candidate for ordinary ContentHost plus private literal descriptor |
| `appendInitialChild` | Link candidate children only |
| `finalizeInitialChildren` | Finish pure validation; no native mount work; return false |
| `shouldSetTextContent` | False; raw text uses the same normalized content-host route |
| `prepareForCommit` | Open one JS journal, not a native begin-transaction |
| append/insert and container variants | Stage ordered `INSERT_BEFORE`, create previously unmaterialized candidates once |
| `commitUpdate` | Diff normalized next values against **accepted** snapshots; stage typed sections |
| `commitTextUpdate` | Stage private content replacement, never structural text mutation |
| remove/container remove | Stage native subtree retirement and descendant JS callback cleanup |
| hide/unhide instance/text | Set renderer-hidden state; preserve declared display and identity |
| `clearContainer` | Retire that container's children; not environment teardown |
| `resetAfterCommit` | Encode, call native once, inspect acknowledgement, then promote publication state |
| `getPublicInstance` | Restricted ref, not HostInstance internals or native pointers |
| `detachDeletedInstance` | Idempotent final JS reference cleanup, not the first retirement notification |

Public React conveniences run generated normalization during render so ordinary invalid props fail before the mutation phase where possible. HostConfig/native ingress still defend their own boundary. Do not assume a `prepareUpdate` hook from older renderer versions exists in the pinned shim. [X01, X02]

### 10.4 Commit coordinator

```text
collect callbacks and changed final properties
remove never-materialized candidates that do not survive the commit
assign local creation ordinals
emit typed creation records and ordered structural mutations
coalesce per-property/layer updates and event-presence changes
stage binding requests/private replacements in their own sections
if no native work: update JS-only callbacks and finish
otherwise commitUiV1 once
if accepted:
    install returned handles in candidates/tokens
    promote touched accepted snapshots and JS ownership links
    install callback changes; remove retired callbacks
    resolve accepted-commit observers
if rejected:
    do not promote accepted snapshots
    fault the root as described below
```

Traverse a new subtree once for initial construction. Do not traverse clean existing siblings to rediscover their native handles. Initial `appendInitialChild` edges must be encoded exactly once when that candidate subtree materializes.

The journal updates proposed JS ownership links during the commit and retains accepted property/resource snapshots until acknowledgement. On native rejection the root is faulted; release its proposed-link state during explicit cleanup rather than attempting to roll Fiber back. No inverse-operation engine or second JS tree reconciler is added.

### 10.5 Failure is not Fiber rollback

A renderer throwing during the mutation phase does not prove that React rolled Fiber back. A native desired-state rejection at this boundary faults the root, leaves the last confirmed physical frame intact, rejects pending barriers and prevents further semantic publication through that root. Explicit cleanup/close remains available. Recovery is an explicit new root/session, not a hidden complete-tree resend.

A later layout/projection/presentation failure is different: React/native desired acceptance already succeeded. Retain desired state and pending work; report the frame error at the barrier. Do not revert HostInstance snapshots or replay the UI batch.

### 10.6 Scheduling, effects and refs

Use `queueMicrotask` for React microtasks and the runtime timer API for **React scheduling**, not native animation. Map discrete native application events to discrete React priority, continuous coalescible events to continuous priority and other work to default priority. Save/restore priority around callback delivery. Implement the current update-priority hooks from the pinned shim.

The renderer does not suspend commits for native paint. The commit-suspension predicates return false; preload reports ready and the readiness wait supplies no suspension. Suspense/Activity hiding is supported through retained hide/unhide, not resource destruction.

Use one non-null root host context per host. Same-host typed portals are permitted; cross-host containers are invalid. Keep the native handle correspondence private; never read undocumented Fiber fields to find a parent or key.

A proposed root API is:

```ts
const root = createReactRoot(host);
const revision = await root.render(<App />);
await host.whenVisible(revision);
```

`render` resolves after a commit accepting that request or a newer superseding render. Intermediate render requests are not guaranteed to become visible. No-op renders resolve with the current accepted revision. Effects may rely on native desired state existing, not on terminal output being complete.

Refs expose occurrence-targeted override/focus operations, diagnostics and explicitly visible geometry queries. They do not expose tree surgery, arbitrary props, builder takeover or detached-node keepalive. Ref operations use the same publication coordinator as React; native input has its own revision stream.

### 10.7 Content hooks and speculative ownership

`useContentPort` and `useContentConnector` create **JS-only lazy tokens**. A committed Content consumer causes the journal to allocate the corresponding native resource, returning its typed handle in the acknowledgement. An unused/abandoned token allocates nothing native. An imperative first use outside render may materialize it through the same session.

Hook cleanup releases the hook-owned resource after its attachment is removed. Caller-owned resources are not disposed by a component merely mounting them. No ownership-escape/retain API is added in this tranche; resources intended to outlive a hook should be created explicitly outside render by their real owner.

Do not put an actual `createSource()` side effect inside `useMemo` and call it speculative-safe. Application item factories create Sources outside render; a future commit-owned Source hook would need the same candidate discipline.

---

## 11. Content integration

### 11.1 Preserve one content implementation

Keep `application/source_store.rs`, the Source mutation validator, native semantic text values, the built-in projectors, and Connector-local delivery/projection machinery. The occurrence refactor changes their **UI attachment and scheduling callers**, not the meaning of accepted Source coordinates or the parser implementation. The existing snapshot contains `Arc<StoredSource>` and separates Source lifetime generation, content lineage, revision, base/end and seal state. Preserve those distinctions. [R09]

Put the existing public payload adapter behind `ContentDataTransport` in `transport/content/transport.ts`. Its sole production implementation remains `ffi.ts`. Move the public wrapper's imports to that seam; do not create another N-API payload implementation. Reuse the existing mutation result, qualified Source identity, annotation bounds and native revision assignment.

A Source append linearizes under the Source lock. Afterwards it notifies affected hosts without retaining that lock. A failed wake does not undo acceptance or turn the result into “please append again.” The result records the accepted revision; the host stores scheduling/frame errors separately.

### 11.2 Precise binding ownership

An implicit `<Content source={s} funnel={f}/>` owns one Port and its current/candidate implicit Connector. An explicit `<Content port={p}/>` mounts caller-owned resources and never disposes its external Source. Implicit and explicit props form a discriminated union.

The `ownershipMode` in §8 is `OccurrenceOwned = 1` or `Explicit = 2`. Occurrence-owned Port/control creation names its owner; implicit Connector ownership follows its Port owner. Explicit resources require an acknowledged JS owner token. Disposal checks the final accepted attachment/selection graph and rejects an in-use resource. A candidate-only resource is destroyed when its unaccepted plan is dropped.

Use this operation split:

| Change | Operation and owner |
|---|---|
| Different Port attached to the occurrence | Structural `ATTACH_PORT` |
| Same Port, different Source or Funnel | Content-control candidate Connector and `SELECT_CONNECTOR` request |
| Existing Source receives more bytes | Existing content FFI append, no UI journal |
| Static React literal changes | Private content replacement section, no topology/state change |
| Wrap/delivery/Markdown options change | New immutable Funnel descriptor and Connector switch |
| Theme changes | Native realization invalidation; do not rebuild the Funnel just to resolve colors |

Compare a normalized Funnel by full semantic value equality, optionally accelerated by a fingerprint. Fingerprint equality alone must not authorize unrelated settings to alias. Do not add a global permanent Funnel registry solely to count an immutable value.

### 11.3 Static text is ordinary content, not a second renderer

`<Text>Hello</Text>` and supported raw JSX text normalize to ContentHost. The implicit resource set contains lightweight block storage, a plain/immediate Funnel and a Port/Connector. No stream producer, pacing timer or Markdown parser is necessary.

`REPLACE_LITERAL` is allowed only on the private occurrence-owned literal destination. It prepares storage using the same content-store validator and installs it with the surrounding UI transaction. Version one accepts exactly `Utf8=1`: UTF-8 plus the existing typed annotation representation, moved into generated content-schema output without changing its record layout. Static styled spans normalize to that text/annotation path. Reject other format values. Do not invent an arbitrary semantic-document wire format in this UI simplification; the future SemanticTextSource family is a separately versioned content feature. Built-in Markdown/diff/ANSI still produce the existing native semantic IR from their Sources. Existing typed-diff validation and native rendering helpers remain behavior oracles during M2 content lowering. The new public React `<Diff>` consumes a Source/Funnel; it is not a source-compatible promise for the old View-returning typed-diff authoring helper. Do not silently approximate one with permissive parsing and claim the two APIs are identical.

A first literal replacement creates its private Source candidate and implicit binding as part of the prepared resource plan. Subsequent replacements preserve occurrence and Port identity, advance private content lineage/revision and invalidate only its demanded content products. Those private Source IDs do not enter the public `sources` argument or become an externally shareable handle.

The private-content buffer is a distinct content lane inside the frontend acceptance envelope, as permitted by V5 §20.4. This is **not** a public `Source.append` fallback and not text stored on the structural node. Both physical entrypoints call the same content preparation/storage routines. Large independently changing documents use public Sources and the existing data lane.

### 11.4 Requested binding versus confirmed projection

A Port needs two meaningful versions, not two competing authorities:

```text
requested binding/version: latest accepted content-control intent
confirmed binding/product: last exact product that reached physical presentation
```

When B replaces A, accept the request for B, prepare B using captured geometry/Source/Funnel/Host Environment, and keep A's exact confirmed product until B is usable. A preparation failure does not blank the destination. Successful receipt of a candidate carrying B promotes that candidate's B, never a newer request C accepted after capture.

Hard-invalid handles, incompatible families, wrong host, invalid configuration and duplicate final attachment reject acceptance. Operating states such as insufficient geometry or not-yet-supported capability retain the accepted request and retry only on a relevant dependency change. Source/Funnel/Port endpoints of an existing Connector never mutate in place.

A UI visibility barrier means the accepted UI revision has been realized **with its specified content fallback/status semantics**. It does not promise that every selected Source has finished smoothing or that a blocked B has replaced A. A Content ref additionally exposes `whenContentVisible(bindingVersion, sourceRevision)`; it resolves only when a confirmed product of that binding covers the requested revision/frontier. It rejects on disposal, superseded binding, unrecoverable error or explicit cancellation. This avoids pretending a general UI revision is an exact content-delivery barrier.

### 11.5 Frame-local Source capture

Capture each demanded Source once per candidate preparation, keyed by full lifetime identity. Reuse that immutable snapshot for newly prepared products of that Source in this candidate. An append during preparation cannot give two newly prepared consumers inconsistent input snapshots.

This does not require equal visible frontiers: independent Smooth/Immediate Connectors intentionally differ. Nor does it outlaw A's older pinned projection while candidate B is blocked. Record those exact inputs; never silently look up the latest cached projection during paint. There is no invented globally atomic transaction over unrelated public Sources.

A missing exact ticket/product is a preparation error. It cannot produce a blank region and a successful frame result. Frame pins may outlive cache eviction; the ticket owns access to its exact immutable product.

### 11.6 Native demand, cold state and disposal

Mounted, visible/near content has execution demand. Hidden or later cold-derived content keeps compact binding identity but has no parser/projection/delivery timer work. Source data can continue advancing. React unmount is not required for cold residency.

On reactivation, default catch-up shows existing backlog immediately and smooths subsequent newly accepted content. Explicit replay remains a separate policy. Retiring the implicit occurrence cancels its deadlines, removes demand and membership, then releases its implicit Port/Connector/private Source according to product pins. External Sources survive. Public Source disposal still rejects live memberships rather than cascading through unknown owners.

### 11.7 Preserve current behavior; do not claim this completes all V5 content work

The inspected baseline has Block/Stream sources and Plain/Markdown/Diff/ANSI Funnels. It already has rich semantic values and persistent Source storage. It does **not** implement every future Document/Rolling/SemanticText public family in the preliminary design. Keep those API expansions out of the occurrence cutover.

Two source/target differences must stay visible:

**UTF-8:** current data ingress validates an accepted payload as UTF-8, while V5 describes a future incremental decoder with replacement/diagnostic policies. M1/M2 preserve current ingress behavior. Arbitrarily split byte-stream decoding is a separate content change, not silently enabled by the UI protocol.

**Markdown stability:** a fixed trailing-window heuristic is not a proof of full CommonMark prefix stability. A reference definition arriving later can affect earlier references. Preserve the current parser's restart/reference context and test incremental output against full parsing for supported syntax. Define “stable prefix” as dependency-closed reusable output, or retain explicit invalidation dependencies; do not assert that every previously closed block is permanently immutable. This is a qualification of a deferred parser algorithm, not a replacement grammar. [X09]

The Source-revision grapheme-index scan and other parser/projection costs remain separately measurable. A new occurrence tree does not prove them O(delta). Smoothing stays Connector-local after semantic interpretation, with native ticks and exact source-rooted frontiers. Do not rewrite the rate controller during this refactor merely to select new default numbers.

---

## 12. Native controls and events

### 12.1 Preserve native mechanics, remove composition-only controls

Keep the concrete TextInput buffer/commands, native key and paste interpretation, scroll offset/extent mechanics, focus traversal and native animation timing. Remove `ViewSlot` instances that exist only as publication targets for custom TS execution scopes. An ordinary React child does not need a hidden native component slot.

The new control API is finite: editor, scroll viewport and animation. Their state is either occurrence-owned or explicitly caller-owned; the same concrete Rust control implementation serves both. No generic Rust application-authoring trait is reintroduced.

| Existing behavior | New owner |
|---|---|
| TextInput text/cursor/selection and commands | Native editor control |
| ScrollPane offset, extent and native scroll input | Native scroll control; later Surface viewport state |
| Static child-scope ViewSlot | Deleted; direct child occurrence |
| Animated ViewSlot | Native animation control over preaccepted frame roots |
| Global key binding and paste interception | Existing native input router with typed caller route IDs |
| Arbitrary product decisions | React/application, not Rust controls |

### 12.2 Editor declaration and control facts

Separate `defaultValue` initialization, explicit controlled replacement, and native edits. A React rerender that repeats the accepted controlled value sends no replacement. A changed value emits `REPLACE_EDITOR_CONTENT`, with content bytes in the content lane, not a state string prop.

Keep current native command, Unicode, cursor-repair and paste behavior as the compatibility oracle. The new React event `onEdit` reports the native editor snapshot/revision after a successful native editing or cursor/selection action. It deliberately avoids promising that the old ambiguously named change event was text-only. A text-only `onChange` convenience filters on text revision, while `onSelectionChange` filters cursor/selection revision. This is an explicit new public event contract in §24, not an unrelated silent fix to the old facade. The old facade's behavior remains unchanged until removed.

A declared replacement is a caller command and does not echo a user-input event. Native user edits advance the editor revision and report semantic output after locks are released. The `expectedEditRevision` argument is optional via the all-ones sentinel; when supplied, a stale controlled edit is rejected before acceptance. Ordinary React controlled props use the latest accepted session revision and explicit replacement ordering, not a guessed asynchronously observed edit revision.

Candidate viewport/cursor geometry is not the current logical buffer. An older presentation receipt publishes its captured cursor/hit geometry but must not replace newer logical text or selection.

### 12.3 Focus and hit testing

Use confirmed frame geometry for spatial targeting. Then validate the target's generation and current desired mounted/hidden/disabled eligibility. A retired occurrence cannot receive input just because an old frame still shows its pixels. A newer generation in the same arena slot cannot inherit an old hit target.

A same-parent keyed move preserved by React preserves controller identity and focus. Reparenting that React implements as unmount/mount does not. Hidden or retired focused content loses active focus; use existing deterministic native traversal for the next eligible target. Do not promise implicit focus restoration on unhide.

Collect event payloads under native ownership, then dispatch outside locks. JS owns callbacks by `(OccurrenceHandle, EventKind)`; native stores a finite subscription mask. Replacing a function while its presence remains true changes only the JS registry.

### 12.4 Animation without frontend frames

Accept frame roots and their resource bindings once. The animation control selects the active frame on a native deadline. Inactive frame roots retain ownership but not visible content demand. No React render, structural batch or property batch is emitted per tick.

For interval `d > 0`, epoch `t0`, and `n > 1` frames:

```text
k = floor(max(now - t0, 0) / d)
active = k mod n
next_deadline = t0 + (k + 1) * d
```

Use checked duration arithmetic. Late wakes skip directly to the appropriate frame rather than replaying missed frames. Empty, one-frame, stopped, hidden and unmounted controls have no periodic deadline. An explicit stop preserves the current frame unless a typed command selects a different one.

Validate attachment uniqueness across all owned frame roots, not only the currently active frame. Do not move the old animation setter's bypass of attachment/lifetime validation into the new API. Sources may be shared through distinct Ports; exclusive Ports/controllers cannot be duplicated.

### 12.5 Event buffering

Reuse the existing event queue and bounded drain strategy. Preserve discrete ordering; coalesce only documented continuous pointer/scroll events with the same target/kind. Put a configurable downward-only admission bound on the host queue, initially 4,096 records and 8 MiB of payload.

On exhaustion, retain already queued discrete events, stop accepting further native input and report `EVENT_BACKPRESSURE`. Do not silently drop keys/pastes/clicks or spin allocating an unbounded queue. Resumption occurs after draining below the limit. This bounds framework storage; it does not claim the operating system can buffer unbounded input while an application is stalled.

Stale-generation queued callbacks are discarded with a diagnostic count, not delivered to a replacement occurrence. Source acceptance order remains independent from event order; a callback that appends content creates ordinary later work.

---

## 13. Frame capture, receipts and failures

### 13.1 Three independent facts

Maintain separate counters for:

```text
accepted_ui_revision   JS-owned semantic publication ordering
pending_work_epoch     all native work, including content/input/environment
visible_frame_id       exact confirmed physical product identity
```

Sources, controllers and property domains retain the smaller revisions needed for dependency checks. Do not create one counter per helper or treat a UI revision as a Source revision.

Replace correlated candidate Options in `HostInner` with one presentation enum:

```rust
enum PresentationState {
    Idle,
    Prepared(PreparedFrame),
    InFlight { frame: PreparedFrame, receipt: PresentReceipt },
    Completing { frame: PreparedFrame, receipt: ConfirmedReceipt },
    Failed(FrameFailure),
    Closed,
}
```

The host separately retains its last confirmed frame metadata and current desired document. A `PreparedFrame` captures UI revision, work epoch, environment subrevisions, exact content products, output/cursor/hit metadata and required resource pins. No mutable `NodeRecord` reference or borrowed JS buffer escapes into it.

### 13.2 Preparation sequence

1. Service an outstanding receipt first. Never prepare a second complete frame while one is in flight.
2. On the renderer driver thread, acquire the host guard and snapshot the current UI/work/control/environment input revisions.
3. Capture demanded Sources once and resolve selected/candidate content from immutable snapshots.
4. Synchronize changed backend layout nodes and prepare layout, presentation, cursor/hit products and any History transfer plan.
5. Reserve all completion bookkeeping. A candidate must be promotable without a fallible lookup of “current” desired products after output succeeds.
6. Store the owned candidate, release guards and submit only its owned data to the physical backend.
7. New desired changes may now advance independently. They set newer pending work; they do not edit the submitted product.

The initial terminal implementation can capture under one host guard because all semantic preparation is native and bounded by the existing work admission. Do not invoke JS or terminal I/O under that guard. Later worker parallelism must preserve immutable input capture and is not a prerequisite for this design.

### 13.3 Exact completion rule

```text
candidate captured: UI 10, work 20
new work accepted:  UI 11, work 21
receipt for 10/20 succeeds

publish visible 10/20
leave 11/21 pending
```

For each processed domain, clear its dirty bit only if the current domain revision still equals the captured revision. Otherwise retain it. The same applies when only a Source/control changed and UI revision did not advance.

A successful prepare is not a successful receipt. Completion commits the captured content selection and layout/hit metadata, not new desired selections or controller state. Frame/product ownership, rather than another semantic-tree snapshot, supplies isolation.

### 13.4 Metadata-only visibility

A masked base change, subscription-only change, or equivalent no-pixel change can advance UI revision without requiring layout/paint. Such work still needs a visibility/barrier completion.

After any earlier in-flight frame finishes, prepare a `NoOutput` candidate referencing the exact currently confirmed physical frame ID and capturing the new UI/work revisions. If physical synchronization is known and the effective dependencies are unchanged, complete it locally without a backend write. Publish the captured metadata and resolve the relevant barriers.

Do not update an old in-flight candidate's revision to the latest desired value. Do not issue dummy terminal writes or force a layout merely to make `whenVisible` complete. If synchronization is unknown, a recovery frame is required instead of the metadata-only shortcut.

### 13.5 Failure contracts

| Failure | Required result |
|---|---|
| Decode/validation/stale handle/cycle/duplicate binding | Reject whole desired commit unchanged; no callback/native publication promotion |
| Reservation/resource preparation failure | Drop plan and provisional resources; no authoritative mutation |
| React mutation-phase native rejection | Fault root, reject pending requests, explicit cleanup; not Fiber rollback |
| Accepted UI, later content/layout preparation failure | Desired state remains; previous confirmed output retained; report error and pending work |
| Candidate Connector blocked | Preserve its request and previous exact projection; retry on relevant inputs |
| Exact projection ticket missing/poisoned | Preparation error, not a successful empty paint |
| Backend proves zero bytes submitted | Candidate can be retried under explicit retry policy |
| Backend may have partially written viewport | Mark synchronization unknown; next viable frame is full viewport recovery |
| Backend may have partially written scrollback | Stop automatic export; retain confirmed prefix plus unknown-suffix error; never replay unknown rows |
| Physical success followed by unexpected finalization problem | Retain Completing state; finish that exact candidate only; never resubmit its bytes |
| Invariant violation/unsafe poisoned core | Fault host and attempt controlled shutdown, not spacer/default success |

Automatic failure of unchanged work blocks repeated automatic attempts. Relevant new input or an explicit retry barrier can retry. No tight retry loop. Error reports carry attempted UI revision/work epoch, phase, code and retryability.

### 13.6 Teardown while output is pending

Close rejects new host ingress and removes future scheduling demand. It cancels deadlines and initiates backend shutdown, but does not free an in-flight writer's buffers. Settle its receipt or receive a definitive writer shutdown result; then finalize/cancel that exact candidate and release pins.

Do not use an arbitrary short timeout to free memory another thread still owns. Report shutdown failure while ownership remains with the worker until it exits. Explicit close is the deterministic API; finalization is a cleanup fallback, not the only terminal-restoration mechanism.

Retired occurrence handles become invalid before physical cleanup. Old output products may remain pinned, but no old callback or control becomes active again. Release implicit Connector→Port/private Source ownership in dependency order; external Sources remain with their owners.

---

## 14. Scheduling and safe concurrency

### 14.1 Keep the portable core separate from the renderer driver

Use `Arc<Mutex<HostCore>>` only for data the compiler can prove Send. `HostCore` owns occurrences, concrete native control state, subscriptions, content bindings, revisions and work queues. A `RenderDriver` owns layout-engine and backend/platform objects on their required thread. Terminal output workers receive owned Send frame data; GPUI/platform operations remain on their event-loop thread.

Do not assume every Taffy configuration or GPUI object is transferable merely because another host field has a mutex. Do not move the old `unsafe impl Send/Sync` assertions to the new types. Add compile-time trait assertions for the actual cross-thread core/frame/event envelopes.

The current runtime contains unbounded-type callbacks and `Box<dyn Any>` output payloads behind the host boundary. In the touched native path, constrain portable callback/payload types to Send or replace erased application-shaped callbacks with the finite native event/control enums already needed by the binding. Retain behavior, not the unsafe assertion. This is a migration requirement, not a claim that renaming the document resolves B02. [R08]

### 14.2 Lock and callback rules

Use this order:

```text
host core
    -> Source registry, only for membership/lifecycle installation
    -> affected Source records, ascending identity
```

The environment scheduler releases its pending-queue/directory guard before acquiring a host. A Source mutator releases Source guards before host fanout. Never hold two host guards together. Qualified JS objects and buffers are inspected before native mutation locks; no getter, callback, await, user destructor or terminal write runs under acceptance locks.

For M1, the legacy adapter executes on the same native driver ownership boundary as the new layout adapter. Its concrete control payloads must satisfy the new ownership rules before a background driver accesses them. A broad unsafe cast is not a migration shortcut.

### 14.3 Native scheduling loop

Reuse `TuiEnvironment`'s pending-host queue, retry blocking and wake latch. Add the minimal native driver integration needed to service it without TS; do not build a second authoritative JS wake graph.

The environment's scheduling task waits for a notification, receipt completion or the nearest live control/Connector deadline. It drains at most the existing 32-host fairness budget per turn and requeues remaining work. A host waiting for a receipt is sleeping, not continuously ready.

Start with the existing active deadline scan per serviced host. Its simplicity is preferable to introducing a global heap and stale-entry protocol without measurements. Inactive/static/one-frame controls register no deadlines. If measured active timer scale later warrants a heap, that is a local scheduler optimization, not a wire or ownership change.

After accepting a semantic/native change, update the pending epoch and enqueue only on the not-enqueued→enqueued transition. After servicing, compare captured/current work and requeue if necessary. Receipt notification uses the same native pending mechanism.

### 14.4 JS is an observer, not the frame clock

Deliver semantic event batches through the existing asynchronous native event lane after locks are released. A native frame with no application event needs no JS callback. Any worker waits using owned native handles; it must not carry an N-API `Env`, raw JS value or borrowed typed-array slice across threads.

The terminal test must mount once, stop JS-side frame/drain activity, then inject native time/content/input and obtain new confirmed output. Smooth ticks, animation and focus/cursor changes must report zero TS→Rust semantic calls. A TS microtask wake hint may remain for convenience, but native correctness cannot depend on it.

### 14.5 Avoid turning this into a new framework

Do not add an actor framework, a general task graph, cross-call UI transaction service, universal cancellation token hierarchy, or per-occurrence mutex. The existing environment queue plus one guarded semantic owner and one renderer driver per host provide the required serialization. The backend's real thread requirements determine placement.

---

## 15. Invalidation and caches

### 15.1 Finite semantic domains

Generate these effect categories from the property schema, reusing current semantic meaning:

```text
PRESENTATION
CONTENT_PROJECTION
LAYOUT_INPUT
INTERACTION_RUNTIME
HOST_ENVIRONMENT_DEPENDENT
STRUCTURE_GUARD
```

Topology additionally invalidates the changed parent lists and old/new geometry. Backend code refines effects into layout/paint work. A property is not structural merely because its layout consequences are large.

Store per-domain revisions and dirty bits on affected occurrences/owners, with host-local deduplicated worklists. Increment once per changed domain per acceptance; coalesced no-ops do not increment. Keep only counters that participate in an actual cache or acknowledgement dependency.

### 15.2 Required dependency map

| Change | Required work | Must not happen |
|---|---|---|
| Own background/border color | Own presentation damage | Frontend subtree publication, semantic reparse |
| Inherited color/theme selector | Native matching descendant style resolution | Rebuilding React components just to recolor |
| Gap/padding/size/grid tracks | Layout input and necessary ancestors; width-dependent content projection | Structural recreation of the same occurrence |
| Child move | Old/new parent topology and layout/hit ordering | Resending unchanged child properties/content |
| Source append | Demanded Connectors, resulting projection/metric work | UI state/topology transport |
| Smooth tick | Visible semantic frontier and its projection/metrics | React reconciliation or reaccepting Source data |
| Native cursor move | Cursor/selection and viewport repair if needed | Complete text replacement |
| Viewport width change | Relevant layout/text wrap caches | Semantic UI reconstruction |
| Host palette refinement | Resolved color/presentation | Markdown parse/layout when metrics unchanged |
| Same-presence callback replacement | JS callback map only | Native subscription write |

### 15.3 Ancestor propagation

For layout-input or child-list invalidation, mark the changed node/parent and its ancestor frontier. Deduplicate already dirty ancestors. Only a backend-proven containment boundary may stop propagation early. Fixed width alone does not prove that child height cannot affect an ancestor.

M1 may conservatively use the current renderer's retained measure/prepare machinery; M2 delegates general-layout caching and dirty propagation to Taffy. Do not keep both general-layout dependency systems after Taffy cutover.

A wide reorder can reposition many children and rebuild the affected parent's derived indexed list. This is necessary native work, distinct from unnecessary frontend retransmission. Counters distinguish links edited, indexed children synchronized, leaf measurements, layout nodes visited and cells painted.

### 15.4 Content and presentation keys

Semantic content keys include Source lifetime/lineage/range/revision and semantic transform policy. Width, theme and reveal frontier belong only in products that actually depend on them. Preserve exact key equality even when a hash indexes the cache.

Use relevant Host Environment subrevisions, not one global revision in every cache. Color capability changes cannot invalidate parsing. Real measurement-environment changes can invalidate shape/wrap/layout.

A frame ticket retains its exact product independently of cache eviction. Keep the existing small Connector working sets initially rather than adding a global LRU over all object kinds. Occurrence caches die with their owner; no path/style/NodeId interners survive from the old publication model without a current semantic owner.

### 15.5 Completion and bounded retention

Capture the domain revisions consumed by a frame. On completion, clear only unchanged consumed revisions. Newer pending work remains dirty. A cancelled or failed candidate cannot mark desired work complete.

Keep at most the confirmed product set and one candidate's necessary differing products, plus the existing bounded per-Connector caches. Further desired commits update current state rather than retaining one semantic-tree snapshot per commit. Resource admission still limits the total desired tree/content. A stalled backend can retain necessary pins indefinitely in time; the claim is a bounded number of frame versions, not time-bounded output completion.

Track live nodes, retiring payloads, live resource memberships, cache payload, cache metadata and arena capacity separately. Reused high-water capacity is not an unreachable-object leak; unbounded obsolete metadata is.

### 15.6 Physical correctness remains independent

The current terminal backend may still diff a full Surface. Damage bookkeeping alone is not evidence of damage-bounded output. Preserve wide-grapheme leader/continuation invariants and full-versus-row-window parity. If a damage clear intersects part of a wide glyph, expand to the complete glyph before repainting.

The DOM-like transport guarantee is about semantic traffic and ownership. It does not assert constant-time layout, terminal I/O or parser work, and it does not automatically close atlas C03–C08.

---

## 16. Current-renderer integration and its deletion

### 16.1 Why a temporary adapter is justified

The baseline's content and control renderers still lower through private Views, and its SceneHost owns real interaction/layout work. Deleting all of that while introducing the first occurrence commit would combine frontend, transport, layout and content-renderer regressions. M1 therefore permits **one private native adapter**, `application/legacy_scene.rs`, with no public compatibility authoring API. [R05, R14]

Its output is disposable renderer input. The document remains the identity, topology, declared property and binding authority. It never calls the N-API View ABI, allocates NativeRefs, manages JS leases or reconstructs TS semantic descriptions.

### 16.2 Adapter interfaces

```rust
struct LegacySceneAdapter {
    // Derived entries indexed by full NodeKey; no independent mutation API.
    recipes: RecipeCache,
}

impl LegacySceneAdapter {
    fn synchronize(&mut self, document: &OccurrenceDocument, changes: &ChangeSet);
    fn prepare(&mut self, capture: &NativeFrameInputs) -> Result<PreparedSceneFrame>;
}
```

`ChangeSet` is a host-local, coalesced set of changed node keys/domains since this adapter last synchronized, not a copy of every UI command and not an append-only event log. If a node is created and removed before synchronization, its generation-qualified entry vanishes without constructing a recipe.

For each shape/topology change, build the affected occurrence recipe and its ancestor frontier using unchanged child recipes. For ordinary presentation changes, read the occurrence's current effective state; do not rebuild semantic ancestors simply to recolor a border. If a legacy factory embeds a geometry field that cannot be read indirectly, classify that field as a **legacy recipe dependency** and rebuild only the native recipe frontier. This temporary backend cost must be measured and deleted at M2, not misreported as structural transport.

### 16.3 Mapping to current private factories

| Occurrence | M1 private renderer mapping |
|---|---|
| Box row/column | `View::native_axis_from_children` / corresponding axis factory |
| Box grid | `presentation::factory::grid` with final ordered child placements |
| Ordinary single-child wrapper | `presentation::factory::container` where behavior-equivalent |
| ContentHost | `presentation::factory::content_host` for its qualified Port |
| Editor/Scroll/Animation | Existing concrete native control expansion |
| Clamp/viewport decoration | Existing private renderer operation selected from typed declared semantics |

These symbols are internal render adaptation, not a new wire vocabulary. The inspected private presentation binding exposes the corresponding factories. [R14]

Attach a private `NodeKey` origin to occurrence-backed recipes. Replace the ordinary state lookup at the renderer read boundary with a small read-only accessor that obtains the occurrence's complete effective value. A recipe's old embedded base must not override the current declared base when an override is cleared.

During the branch migration, adapt the existing `StateFrameView` users to this read-only interface. Do **not** create a second mutable ViewStateRegistry synchronized from the occurrence document. Legacy content-internal fragments without an occurrence origin retain their own content-local style/layout values. The current capture map's useful immutable read semantics can be reused, but its ordinary attachment identities and mutation owner are removed. [R14]

### 16.4 History integration during M1

Each current History unit owns an internal root container. The adapter supplies that unit's derived legacy recipe. Preserve existing unit IDs, boundaries, append/freeze/discard rules and native transfer behavior until the separately approved Surface migration.

Split any existing History/control operation that mutates and then flushes into `prepare desired change`, `install prepared change`, and `schedule frame`. Only the first two can participate in UI desired acceptance. Terminal output is never part of the UI apply function.

The temporary History API uses typed portal containers owned by the current History unit, not arbitrary retained View values. Finalizing a unit requires an explicit final component/root replacement under the existing unit contract. It is not equivalent to Source seal or immutable presentation. No hidden finalization is triggered merely because an application calls a resource complete.

Nested state/content metric changes beneath a History root dirty that unit's layout dependency. Do not retain the shortcut of watching only a root ContentHost. This is a required dependency on the new route, not a claim that every atlas C01 counterexample is already reproduced.

### 16.5 Exact M1 and M2 deletion gates

At **M1**, remove production callers and implementations of custom TS composition, semantic UI NodeIds, immutable View materialization, native ViewRef/path/style publication tables, root/temporary leases, remote builders/edit transactions, and separately allocated ordinary ViewState. Retain the one native legacy adapter and renderer internals it actually calls.

At **M2**, remove `legacy_scene.rs`, its recipe cache/origin adapter, the old ordinary state-read implementation, old general UI Scene resolution and redundant measure/prepare/index machinery. Port remaining **content-local** View lowering as specified in §17.3 before deleting the last required renderer helpers. Concrete input/control/terminal/provenance helpers remain where they still own behavior.

A deletion is complete only when package exports, generated files, schema entries, tests, benchmarks, docs and imports no longer depend on the removed protocol. Do not keep obsolete tests alive by rebuilding the deleted abstraction in a test-only implementation.

---

## 17. Taffy and the final rendering boundary

### 17.1 Selected integration

Use **Taffy 0.12.2**, pinned, with `default-features = false` and `std`, `taffy_tree`, `flexbox`, `grid`, `content_size`. Its inspected tagged source contains the high-level `TaffyTree` storage, child/parent lists and cache. No CSS parser, general property bag, float layout or HTML compatibility is needed. Keep it behind `presentation/taffy.rs`. [X05]

Choose the high-level API rather than implementing a parallel low-level cache/traversal engine for the first version. The native occurrence document still owns semantics; `NodeKey → Taffy NodeId` is a backend mapping and can be discarded/rebuilt. It never crosses TS/Rust.

Sync only changed topology/geometry before a layout. Construct a complete native Taffy Style only when that occurrence's effective geometry changed; that local conversion is not complete-style **transport**. Skip `set_style` when normalized geometry is equal, avoiding unnecessary dirtiness.

### 17.2 Topology synchronization algorithm

Maintain the last synchronized child-list revision per parent and full NodeKey in each leaf context.

1. Create missing layout nodes for new occurrences and synthetic internal layout containers.
2. Collect affected old/new parents. Remove their old Taffy child edges before installing any final child lists; this prevents a later old-parent update from detaching a child already moved to its new parent.
3. Build each affected parent's indexed child list once from canonical links and install it. Untouched parents are not traversed for synchronization.
4. Apply changed styles/leaf contexts, then remove retired layout nodes in postorder.
5. Run Taffy layout for affected roots with the current constraints and content measure callback.
6. Compare resolved rectangles/hit order with prior physical metadata and mark relevant paint/cursor damage.

Validate at the document boundary, not by duplicating cycle checks inside every backend call. A backend mapping inconsistency is an invariant failure. Initial synchronization is O(all new nodes); a changed parent list is O(its degree). Taffy's own layout can visit a larger dependency frontier.

### 17.3 Content measurement and removal of the last View lowering

At M2, add `content/text/terminal.rs` as a direct consumer of the **existing** semantic text IR and text-render policy. It produces measured semantic runs/block boxes and exact paint products, not general UI Views. Preserve source ranges, semantic role/context and annotations through measurement and paint.

Use the same Taffy algorithms for content's spatial containers where needed. A Connector may own a projection-local Taffy subtree for its semantic blocks; this is a width-specific derived content product, not UI topology or another general-layout algorithm. Main occurrence layout treats ContentHost as a measured content leaf. This hierarchical measurement is distinct from running two full competing layout engines over the same UI tree.

| Semantic content | Direct M2 realization |
|---|---|
| Paragraph/heading | Inline semantic run leaf; existing Unicode/line-break/wrap machinery; heading roles resolved at paint |
| Quote | Container plus content-owned quote marker/indent, not a structural Hanging host kind |
| List item | Taffy row/grid marker region plus flexible body; continuation indentation inside text wrapping |
| Code/raw block | Literal run layout with the existing code-label/wrap policy and provenance |
| Table | Taffy grid over semantic cell content, preserving spans, caption, alignments and policy |
| Rule | Content-owned measured line/paint primitive |
| Typed diff | Validated semantic rows/roles and optional gutters, not structural Diff |
| Images unsupported by current text host | Preserve existing alt-text behavior; no invented native image support |

Inline shaping/wrapping remains a content algorithm, not Taffy's job. Reuse it rather than rewriting Unicode handling. Parsing remains width/theme independent. Measuring the same exact content/constraint key is pure and does not advance smoothing, consume input, mutate Source or emit events. Delivery frontier selection occurs once before measurement.

The leaf callback uses definite known width when supplied; otherwise it implements Taffy's min-content/max-content/definite-available-space requests explicitly. Min-content uses the longest unbreakable unit under the selected wrap policy; max-content uses the unwrapped maximum hard-line extent. Returned size respects known dimensions. Empty content and zero width follow explicit tested behavior, not division-by-zero or sentinel wrap widths.

### 17.4 Terminal rounding policy

Taffy computes in cell-space scalars for terminal. Convert **absolute accumulated edges** with one deterministic nearest-edge rule, then derive widths/heights by edge subtraction. Do not independently round each sibling width; that can create gaps or overlaps.

For wrapped terminal text, choose the integer wrapping constraint as `floor(max(0, resolved logical content width))`. Retain the exact projection measured at that constraint for painting; do not rewrap at a possibly one-cell-larger rounded rectangle after height was chosen. The rounded rectangle may contain one spare column. This is the selected terminal quantization policy, with fractional-layout goldens required before M2 cutover. It avoids an unproved endless layout/rewrap fixed-point loop.

Padding/clip/cursor conversions use the same edge policy and checked physical limits. Existing integral layouts should remain identical unless an intentional Taffy semantic change is documented and approved. A backend cannot silently wrap a large coordinate into u16.

### 17.5 Paint and interaction

The layout adapter emits geometry/clip/hit metadata; native semantic styling and content products emit cells. Keep paint order tied to final document order. Preserve wide glyph integrity at clips and dirty boundaries.

A control's logical input state stays separate from its candidate viewport feedback. Updating candidate bounds cannot overwrite newer desired input when an older frame completes. Apply native layout feedback within the captured prepare or as new native work, with explicit convergence limits inherited from the current host until they can be removed—not silent repeated React commits.

### 17.6 GPUI integration rule

GPUI consumes the same accepted occurrence/property/content semantics and lowers them into ephemeral Elements for its own frame. It uses its Taffy-backed layout with GPUI text measurement. Do not force terminal cell geometry into GPUI, serialize GPUI Elements over the bridge, or run the standalone terminal Taffy tree before GPUI lays out the same UI again. [V5 §§24–25]

Stable GPUI element identity derives from the full occurrence handle, including generation. Its platform/editor/accessibility implementation is separate V5 work; it must not require a new semantic View DAG, props bridge or Source transport.

---

## 18. Surface, History and GPUI integration

### 18.1 Separate milestones, fixed ownership

M1 preserves current History behavior through the private adapter. M2 replaces general layout and content-to-View lowering. The **component-only Surface migration** then removes the remaining special History semantic owner. GPUI is a subsequent backend. This ordering is explicit: neither wholesale Surface replacement nor implementing a GUI is required to validate direct occurrence mutation.

The future Surface is the existing `Scroll` occurrence behavior plus spatial state, not a new content engine. It contains ordinary child occurrences and owns viewport/clip, scroll offset, follow state, anchors, residency and extent caches. It does not own Source bytes, Markdown policy, smoothing or application completion.

### 18.2 Initial Surface implementation

Start with all semantic children and their Taffy layout retained; cull offscreen paint. Do not add React unmount virtualization, a general extent-estimation framework or global semantic cache in the first Surface version. This follows V5 §18.8 and minimizes new state machines.

Next add visible/near/cold-derived projection residency only after the retained baseline works. Default overscan is two viewport extents in each direction; clamp to available content. Focus, selection, pointer capture or explicit navigation promotes an otherwise cold target. Cold extent reuse is valid only for the constraint/measurement key under which it was measured. A width change invalidates it.

Every resident occurrence responds to current theme/state/environment regardless of age. A Source seal permits parser finalization, not rendering freeze. Old visible border/theme regression tests therefore target normal presentation invalidation, not an `unfreeze` API.

### 18.3 Follow and anchor algorithms

Follow state is `Disabled`, `Following` or `SuspendedByUser`. When Following, set offset to `max(0, contentExtent - viewportExtent)` after layout. A deliberate user scroll away from the end suspends follow. Returning within the physical end threshold, or explicit `resumeFollowEnd`, resumes it. Initial thresholds are one terminal cell and two GPUI logical pixels, kept in backend policy rather than wire semantics.

When not Following, choose a visible focus target, then active selection/find target, then a suitable first visible child. Capture `(full NodeKey, local offset, old block-start position)`. After layout, adjust scroll by the new-minus-old position and clamp. If removed, select the nearest surviving visible successor, then predecessor, then clamp. An explicit user scroll in the same native input batch takes precedence over preserving the previous anchor.

This follows the stable-anchor principle, not full browser CSS scroll-anchoring compatibility. [X10]

### 18.4 Terminal scrollback is an external export

An Iyon-controlled resident Surface remains mutable. Terminal scrollback already written outside that viewport cannot generally be recolored/reflowed by mutating the document. Keep an explicit physical export ledger with confirmed prefix and captured width/style inputs; do not disguise this irreversible fact as a semantic frozen occurrence.

During M1, retain current History transfer behavior and its existing explicit caller controls. The later Surface release must expose terminal export as a backend-specific physical operation with explicit eligibility, not automatic export on Source seal or application completion. The selected initial Surface policy is **no automatic export of still-mounted mutable children**. Existing applications requiring inline scrollback keep the documented History adapter until their export calls are ported; that adapter cannot remain a general-layout/immutable-View authority after M2.

Export accepts a captured immutable output product and tracks acknowledged rows. A sink failure with unknown partial writes disables further automatic export for that session and reports `HISTORY_SYNC_UNKNOWN`; no automatic replay or terminal clear occurs. Closing/reopening a session does not promise reconstruction of external scrollback. This is a deliberate physical limitation, not a reason to weaken desired-state correctness.

### 18.5 Backends do not leak into application policy

Keep the framework generic. “HistorySurface,” conversation item factories and tool/message state are application components. Rust owns only reusable spatial/content/native mechanics. A log viewer/editor/dashboard can use the same occurrence, Source, Surface and input model without agent-specific fields.

Most Host Environment changes remain native; explicit React observation uses one revisioned external store with selector support. Services such as clipboard return authoritative operation results; capability Unknown is distinct from Unsupported. No new global capability map of arbitrary keys is introduced in the occurrence refactor.

---

## 19. Six end-to-end traces

### 19.1 Initial mount

```text
React evaluates <Column><Text>Ready</Text><Editor /></Column>
  -> JS-only Box, ContentHost and Editor candidates
  -> initial child links remain JS-only
React commit
  -> local creation ordinals, initial nondefault properties
  -> structural creates/edges/control attachment
  -> implicit Port/control creation; literal in content buffer
native prepare
  -> decode, qualify, reserve, validate final tree/resources
native apply
  -> publish all handles and UI revision N once
acknowledgement
  -> HostInstances store handles/accepted snapshots
native frame
  -> current adapter or Taffy + content measurement -> owned product
receipt
  -> visible N and exact product pins
```

Initial mount necessarily transmits all newly introduced semantics once. Abandoning the React render before commit allocates no native resource. Failed preflight publishes neither half the tree nor a literal Source membership.

### 19.2 One style change

`background: blue → green` on occurrence K normalizes to one PropertyId/value delta. No create, parent list, child content or sibling prop crosses. Native preflight writes K's declared layer and computes effective effects. A masked base change updates accepted state but can use NoOutput completion; an effective color change marks presentation only.

React commit count may advance because the prop changed; native layout/parsing must not advance solely for an ordinary local background. An inherited style or theme selector can require a native descendant walk, still without resending those descendants.

### 19.3 Keyed move

Fiber preserves the host instance and calls `insertBefore(parent, child, anchor)` or append for the chosen placement. The encoder supplies existing qualified handles. Rust validates the anchor/host/cycle, relinks the same occurrence, and preserves its native control identity.

Only affected parents/ancestors are dirtied. Taffy may update positions of many siblings. Those layout consequences do not create new UI handles or transmit sibling styles. Some React reorder patterns emit several placements; record the actual callbacks rather than claiming a custom minimal edit script.

### 19.4 Source append

```text
source.append(chunk)
  -> existing content-data ABI
  -> Source mutex validates and installs new persistent storage/revision
  -> unlock; notify selected/preparing affected Connectors/Hosts
  -> native pending epoch and wake
  -> demanded Connector captures Source snapshot
  -> semantic parsing/delivery/projection as needed
  -> changed content metrics may dirty layout
  -> candidate output and exact receipt
```

React renders/commits: zero. UI topology/state bytes: zero. Source payload is the accepted chunk/annotations, not accumulated content. This does not assert that every existing parser/indexing stage is already proportional to the chunk.

### 19.5 Native input or tick

A decoded key targets the native focused editor. The native command updates logical editor state and revisions, emits a typed event if subscribed, and schedules native work. React may later react to the high-level event, but it is not required to interpret the keystroke or render the first native response.

An animation/smoothing tick advances its native selection/frontier and deadline. It uses already accepted roots/Source data. A static or hidden controller has no tick deadline. Each native-only tick records zero UI calls and zero TS→Rust bytes.

### 19.6 Teardown with a frame pending

Frame F captured node `(slot=8,generation=3)`. Before its receipt, React removes that subtree in accepted revision N+1. Its handle becomes invalid; subscriptions and future demand disappear. F still owns its exact output/products. New key `(8,4)` cannot receive F's input.

When F succeeds, promote F's captured revision only, release superseded physical pins and service N+1. When F fails with uncertain output, enter physical recovery; do not restore the removed node or replay its old UI commit. On host close, retain writer-owned buffers until the writer exits and release resources only through their actual owners.

---

## 20. Keep, simplify, replace and delete map

All paths are relative to the pinned repository. Existing paths below were identified through current source and the inventory; **new destinations** are the proposed layout in §5. A folder deletion means remove obsolete responsibilities/callers, not blindly delete files still used for independent content or physical behavior.

| Existing location/responsibility | Action | Exact replacement / retention reason |
|---|---|---|
| `packages/iyon-tui/src/composition/` scopes, tracked state, child owners, publication targets | **Delete at M1** | Fiber scheduling/keys; React state/external stores; direct HostConfig callbacks |
| `src/api/view/` immutable UI authoring and semantic NodeIds | **Delete as public/production authoring at M1** | React components and finite normalized props; move useful passive enums to schema-owned types |
| `src/transport/structural/retained-dag.ts` | **Delete at M1** | Direct occurrence handles and one UI journal; no materialization correspondence |
| `retained-path.ts`, `native-view-abi.ts`, structural encoding/helpers | **Delete at M1** | Generated UI op/property encoder; no path reconstruction or fallback transport |
| Ordinary `src/transport/state/` session and ViewState lifetime wrappers | **Replace at M1** | Occurrence-addressed declared/override fields and same commit coordinator |
| `src/runtime/runtime.ts` execution/publication integration | **Simplify** | Keep host lifecycle, barriers, event delivery; remove custom composition and mandatory TS frame pump |
| `src/runtime/native-resource-registry.ts` | **Simplify** | Qualified real content/controller ownership only; no semantic View/root lease graph |
| `src/api/content/retained.ts` | **Keep/adapt** | Source/Funnel semantics; import typed ContentDataTransport; lazy implicit React ownership |
| `src/transport/content/ffi.ts` | **Keep** | Sole public bulk data adapter; qualified same-image Source mutation |
| `crates/iyon-tui-native/src/tui/view_abi.rs` | **Delete at M1** | `tui/ui_commit.rs` qualified batch entrypoint; core document owns nodes |
| Old generated `view_abi_*` / C View ABI headers and TS structural calls | **Delete superseded outputs** | Generate new UI schema once; retain only live content ABI symbols |
| `crates/iyon-tui-native/src/tui/view_state.rs` and ordinary state envelope | **Replace** | Generated property decoder and occurrence commit |
| `crates/iyon-tui/src/retained_state/{record,registry}.rs` ordinary owners | **Delete at M1** | Occurrence declared/override storage, resource pins and frame metadata |
| `retained_state/{effects,geometry,presentation,capabilities,occurrence}.rs` | **Consolidate** | Move useful semantic rules into `occurrence/properties.rs` and generated descriptors; remove separate attachment identity |
| `retained_state/capture.rs` | **Replace/simplify** | Read-only captured occurrence/property access; no rival mutable registry |
| `application/host.rs` | **Simplify/integrate** | HostCore/document plus explicit frame coordinator; split correlated candidate options |
| `application/kernel.rs` | **Keep native mechanisms, shrink** | Concrete input/control execution; remove generic composition-only mount resolution after M2 |
| `application/environment.rs` | **Keep/improve** | One Source registry/pending queue and native driver; remove unsafe threading assumptions |
| `application/{content,source_store}.rs` | **Keep/adapt boundary** | Resource preparation/installation, exact product capture, persistent content ownership |
| `scene/{host,resolve,resolved}.rs` and general View expansion | **M1 adapter, delete superseded at M2** | Direct document→layout/input realization; retain only independently needed control behavior |
| `presentation/{ir,factory,api}.rs` general UI View model | **Delete superseded M2 callers/types** | Iyon occurrence props; content semantic IR→content projection; no View staging API |
| `presentation/layout/` general allocator/caches/indexes | **Replace at M2** | Taffy general layout/cache; retain Unicode wrapping/physical helpers separately |
| `content/text/` projectors/semantic types/policy | **Keep** | Content is a genuine independent semantic/execution model |
| TextRenderer's semantic→View lowering | **Replace at M2** | Direct content block/run projection in `content/text/terminal.rs` |
| `controls/` editor/scroll commands | **Keep/adapt** | Native behavior and event contracts, not React keystroke handling |
| `history/` mixed semantic owner | **Temporary then replace at Surface gate** | Component-only Surface; physical export ledger remains terminal-specific |
| `history/native/`, `terminal/`, `physical/`, semantic theme rules | **Keep/refine only when touched** | Real terminal output, receipts, glyph safety, styling and restoration |
| `tools/tui-abi-gen` | **Keep/simplify outputs** | Finite generated UI/property/content ABI, one source of packing truth |
| Old path/lease/NodeId protocol tests and benchmarks | **Delete or port real behavior** | Test identity/delta/receipt/content behavior through actual new route, not obsolete machinery |

**No responsibility is deleted merely because its name contains `retained`.** Deletions target redundant publication ownership; useful caches and native execution remain explicit.

---

## 21. Implementation tranches

Each tranche is a coherent branch/commit slice with a usable checkpoint. Use `agent/dom-occurrence-runtime` or an equivalent isolated branch. Do not edit `main` directly. The owner approval in §24 covers the intended cross-file/API/dependency changes; unrelated work remains excluded.

### T0 — Baseline and behavior map

1. Check out the pinned main revision; record any subsequently merged delta before rebasing the plan.
2. Read the current guide, atlas reconciliation/findings, public package manifests and `AGENTS.md`. Do not restore deleted generic Rust application APIs named in historical text.
3. Record baseline actual-route counters/screens for initial mount, one color/gap change, keyed reorder, streaming content, native editor/tick and delayed receipt/close using existing harnesses.
4. Record Rust/TS/generated/test LOC and the publication owners to be removed. Count maintenance burden rather than imposing an invented percent-LOC target.
5. Run applicable existing baseline checks; record known lint/debt separately. Do not fix mechanical lints as part of this architecture tranche.
6. Write a small migration checklist in the PR description linking every existing consumer/example/control/history route to its porting tranche.

**Exit:** reproducible baseline evidence and named public changes. No production behavior changed.

### T1 — Finite schema and occurrence core

1. Add `tools/tui-abi/ui_abi.toml` and the generator path for §8's header, opcodes, values and property descriptors.
2. Migrate existing finite property enums/normalization/defaults/effects into the manifest; generate a property coverage table from it. Use existing semantic types rather than inventing a CSS object model.
3. Add `occurrence/{mod,arena,tree,properties,commit}.rs` with generation-safe storage and the specified link operations.
4. Implement sparse preflight, final owner indices, local creation references and reserved apply. Keep content/control preparation behind concrete owned plan records, not trait-heavy plugin transactions.
5. Implement rejection error codes and exact acknowledgement layout; do not yet call the renderer.
6. Extend focused core tests for topology/override/atomic rejection; add generated malformed-wire fixtures at the decoder boundary, not duplicated at every helper.

**Exit:** internal headless document operations produce the expected final tree/props and reject failed batches unchanged. No native frame or JS fallback is required.

### T2 — Qualified native ingress and resource preparation

1. Add `crates/iyon-tui-native/src/tui/ui_commit.rs` and expose its method through the qualified host wrapper/binding seam.
2. Validate native object/backing-buffer identity, offsets, lengths and non-shared policy before Rust borrowing. Allocate acknowledgement storage before apply.
3. Split current content/controller setters into fallible prepare and reserved installation where this batch requires atomicity. Leave parser/layout/output out of desired commit.
4. Add private literal creation/replacement and literal Funnel switching using the existing content storage and Connector implementation.
5. Add direct final owner indices for Ports/controllers. Implement implicit versus explicit disposal and typed cross-host/cross-kind failures.
6. Replace unsafe View-runtime pointer access on the new route; qualify actual Send ownership before a native driver shares host data.
7. Extend the existing ABI/binding/ownership checks and rebuild/stage the native addon before Bun boundary tests.

**Exit:** one actual N-API call accepts typed UI/resource work; rejection leaves UI/Source memberships unchanged; no old View ABI participates in the new route.

### T3 — Minimal React renderer

1. Add pinned React/reconciler dependencies and the `@iyon/tui/react` entrypoint; do not rename the package.
2. Implement `instance.ts`, `host-config.ts` and `commit.ts` with JS-only speculative creation and the callback table in §10.
3. Implement Box/Row/Column/Grid conveniences and Content/Text; passive values use generated normalization. Add native control wrappers only over the existing control semantics.
4. Implement accepted snapshot promotion, local-handle acknowledgement, callback-presence tracking, lazy Port/Connector tokens and root fault behavior.
5. Wire root acceptance promises and separate UI/content visibility barriers. Exercise no-op and callback-only commits.
6. Port the workspace consumer fixture to React and use public imports, not native-private escape hatches.

**Exit:** initial mount, prop changes, raw/static content and keyed reorder traverse the new production bridge; abandoned render creates zero native resources; no-op rerender makes zero native semantic calls.

### T4 — Current renderer, controls and exact frame state

1. Add the one-way `application/legacy_scene.rs` adapter and read-only occurrence state access. Do not maintain a mirrored ViewStateRegistry.
2. Feed current native controls/ContentPorts/History roots from canonical occurrence ownership. Remove child-scope publication slots on the new route.
3. Extract `application/frame.rs`; replace candidate Options with explicit state and exact captured stamps/products.
4. Implement NoOutput metadata completion, exact dirty clearing and in-flight desired advancement.
5. Split History mutation from flush; integrate deferred retirement and partial-output failure rules.
6. Native-drain the existing environment queue and deadline service with no required TS pump; keep platform drivers on their owner threads.
7. Verify input/paste/global routing, animation identity, content switch failure, old-frame receipt after new desired work, and close while pending.

**Exit:** usable React application on the existing terminal renderer, including controls/content/History behavior, with direct native mutation and exact receipt semantics.

### T5 — M1 cutover and publication deletion

1. Port remaining in-repository public examples/consumers and document the external consumer migration requirement.
2. Switch the canonical production entrypoint to React. Do not provide a permanent immutable-View-to-occurrence compatibility reconciler.
3. Remove obsolete TS composition/View publication and native ViewRef/build/edit/path/lease tables and their exports/generated outputs.
4. Remove ordinary ViewState allocation and its parallel attachment/lifetime tables; move useful field semantics to the occurrence owner.
5. Port behavior tests and benchmarks; delete assertions whose only purpose was proving the removed protocol.
6. Run the full M1 gate, compare real counters/output and review the whole diff. Update both living overview and agent inventory with exact SHA/source links.

**Exit — M1:** only the explicitly named native renderer/History adapters remain. The new frontend/transport is the only production UI route.

### T6 — Direct terminal Taffy integration

1. Add the pinned Taffy dependency/features after approval. Implement `presentation/taffy.rs` as renderer-driver-owned derived state.
2. Generate/implement the selected Iyon Flex/Grid property semantics; validate unsupported properties explicitly. No raw Taffy Style crosses the bridge.
3. Implement changed-node/style and two-phase parent-child synchronization, content leaf context and exact rectangle/hit output.
4. Implement deterministic cell-edge rounding and content wrapping constraint rules from §17.
5. Compare separate development builds against baseline integral-layout fixtures, then approve intentional new Flex/Grid semantics explicitly.
6. Keep general layout in one engine after cutover. Do not retain the old allocator for ordinary Boxes because one case differs.

**Exit:** Box/control layout runs directly from the occurrence document, preserving content/frame ownership and delta traffic.

### T7 — Content lowering and M2 deletion

1. Implement direct semantic block/run projection in `content/text/terminal.rs`, retaining the current projectors/IR/style policy.
2. Port paragraphs, lists/quotes, code, tables, rules, typed diff, annotations and alt text using §17.3. Reuse Unicode/wrap helpers and Taffy for general spatial containers.
3. Verify text/Markdown/diff/ANSI output and hanging-indent behavior through the common ContentPort boundary.
4. Remove the last general UI View recipes, legacy ordinary state-reader and custom general-layout allocator/index machinery.
5. Remove the migration selector/adapter and generated/dead-code residue. Keep only explicitly independent physical History mechanics pending Surface migration.
6. Run full correctness/native platform checks and representative performance comparisons, then refresh both architecture documents.

**Exit — M2:** direct occurrences→Taffy/content→presenter is the only general UI rendering route. No old Iyon View semantic layer or renamed Text/Hanging host kind survives.

### T8 — Separately scheduled Surface and GPUI work

Use §18 and V5's H–M tranches: component-only Surface and anchoring; paint culling before cold-derived optimization; typed Host Environment integration; external application port; then GPUI. These do not gate starting T1 or reaching M1. Their acceptance gates include removal of the remaining History compatibility owner and equivalent shared application semantics across physical hosts.

Do not implement new Source families, a new Markdown parser, arbitrary virtualization, accessibility adapters or complete GUI services merely to finish a DOM runtime PR. Their integration boundaries are fixed here; their own preliminary V5 feature work remains separately scoped.

---

## 22. Verification and performance gates

### 22.1 Reuse the repository's verification surface

Use existing Rust unit/native-host tests, proptest, Bun package tests, `packages/tui-consumer-fixture`, the native smoke path and performance counters. Add a few focused regressions for the new boundaries. Do not create a parallel fake renderer harness to avoid testing production integration.

Fast checks after a relevant slice:

```sh
cargo fmt --all -- --check
cargo check --workspace --all-features
cargo test -p iyon-tui --lib --all-features
bun run check:tui-abi
bun run typecheck
bun run native:stage
bun run check:tui-declarations
bun run check:tui-binding
bun run check:ownership
bun run native:smoke
```

Use narrower existing test filters while editing; the listed group is the tranche integration gate, not a requirement to rebuild everything after each line change. ABI/declaration/binding gates must evolve with the approved public schema change, not be disabled.

Full checkpoint checks include `cargo test --workspace --all-features`, the strict existing Clippy gate, TS lint, all package/consumer Bun tests, generated-file cleanliness and the existing Linux x64/macOS ARM64 native viability matrix. Report baseline lint failures honestly and keep correctness tests runnable; do not weaken lint solely to obtain green CI. [R12, R13]

### 22.2 Small set of strong witnesses

| Owning boundary | Required witness |
|---|---|
| Occurrence core | Move/self-anchor/wrong-anchor/cycle/descendant rescue; rejected multi-op batch leaves accepted state unchanged |
| Handle boundary | Wrong host/type/stale generation; slot reuse cannot revive old input or disposal |
| State semantics | Masked base change, override clear reveals newest base; last-write coalescing and true no-op |
| N-API ingress | Truncated/overflowing records, nonzero offsets, wrong backing buffer, schema mismatch, result allocation before acceptance |
| React | Speculative abandonment; initial children exactly once; subtree cleanup; hidden/unhidden; no-op and function-only callback change |
| Content | Static replacement content-only; external Source survives unmount; shared Source independent Connectors; failed switch retains old exact product |
| Native execution | TextInput/paste/key behavior; animation/smoothing proceeds without JS frame pump; no static timer |
| Presentation | Frame N in flight while N+1 accepted; receipt promotes only N; new dirtiness survives; metadata-only revision completes |
| Teardown/output | Remove/close with pending receipt; no stale callback/UAF; partial unknown scrollback is not replayed |
| Layout/content | Golden integral layout; fractional rounding; wide glyph clips; list/quote/hanging/table/code/diff parity |

Each test belongs at the narrowest boundary that owns its guarantee. Repeat across Rust/Bun only when verifying a genuinely different language boundary, not to increase counts.

### 22.3 Exact transport budgets

Instrument the actual `commitUiV1` encoder/entrypoint and content FFI. Count sections separately even though they share a call.

| Scenario, after initial mount | Expected semantic traffic |
|---|---|
| Same normalized React output | 0 native UI calls, 0 semantic bytes |
| One local background or gap change | 1 state property delta; 0 structural/content payload |
| Callback function changes with same presence | 0 native semantic calls |
| React-selected existing-node move | Only placements React emits; no unchanged property/content resend |
| Static text replacement | Content descriptor/changed payload only; same occurrence/Port |
| Source append | Content mutation and new payload only; 0 React, 0 UI state/structure |
| Smooth/native animation tick | 0 React, 0 TS→Rust semantic calls/bytes |
| Native environment recolor | 0 TS→Rust UI payload unless an explicit application observer changes composition |

Treat violations as correctness/architecture failures, not merely benchmark regressions. Initial mounting and explicit host recreation are the only ordinary reasons to send all new UI semantics. Error recovery must not introduce a hidden full-tree serialization fallback.

### 22.4 Timing and memory comparison

Measure baseline and replacement on the same machine/build profile/Bun version, using the actual staged addon and recorded SHA/schema hash. Warm both runs, alternate order and retain raw samples. Report p50/p95/p99, absolute times, allocation/retained-byte counts, frontend callback/normalization time, native acceptance, layout, content projection and physical output separately.

Required workloads: a large stable tree with one leaf style change; wide keyed reorder; deep local insertion/removal; one Source at multiple widths; steady/burst Markdown append with and without smoothing; native editor and animation; repeated mount/unmount with delayed receipts; realistic resize/theme/scroll behavior. Use existing traces/fixtures where possible.

Acceptance policy: all exact traffic/lifetime witnesses must pass. Investigate any repeatable hot-path p95 regression exceeding **both 15% and 0.10 ms**, and any repeatable multi-millisecond regression regardless of a favorable average. These are proposed review thresholds, not established baseline performance. Do not hide a regression in end-to-end numbers by moving work into another unmeasured stage. Accept only with a documented cause and owner-approved tradeoff or a fix.

Compare shared-resource versus duplicated-occurrence memory explicitly. Sharing a Source remains cheap; rendering a shared declaration twice necessarily creates two independent occurrences. Measure this rather than promising lower memory for every workload.

### 22.5 Complexity acceptance

Report before/after publication owners, semantic-to-semantic translations, independently mutable registries, transport methods, lifecycle state machines and handwritten/generated/test LOC. There is no arbitrary minimum deletion percentage. The non-negotiable deletion list in §20 is the meaningful gate.

A design that merely renames `NativeRef` leases or adds a permanent old-View adapter fails even when benchmarks are green. A smaller implementation that breaks content-only updates or exact receipts fails even when LOC is attractive.

---

### 22.6 Design checks executed for this handoff

A standalone Python reference model compared the specified sparse sibling-link transaction algorithm with an independent child-list oracle. It ran **24 deterministic seeds, 9,600 batches and 28,716 attempted operations**. All comparisons passed: 3,667 batches were accepted and 5,933 rejected batches left committed state unchanged. Eleven directed checks covered cycle rejection after a property write, wrong-parent self-anchor, final-orphan rejection, descendant rescue, no-op writes, the seven-record move bound, coalescing, masked-base semantics, captured receipt stamps and generation exhaustion.

These are finite **design-model** checks, not production Rust/React/ABI tests, a formal proof, or benchmark evidence. The model's whole-state inspection/copies are oracle conveniences, not proposed runtime implementation. The accompanying research bundle contains `research/check_occurrence_model.py` and `research/model-results.json`; the results are also stated here so the Markdown stands alone.

No runtime implementation was made and no new Iyon build/CI pass is claimed. The implementation gates above remain required.

---

## 23. Findings disposition

This table is a **design disposition**, not a claim that a historical risk was reproduced or fixed. Preserve the atlas's confidence distinctions. Close an issue only after its replacement path and required evidence exist. [R11]

| Findings | Effect of the selected design | Required closure evidence / independent work |
|---|---|---|
| A01 content seam | Restores typed ContentDataTransport around the one existing FFI implementation | Public wrappers import the seam; no second payload adapter |
| A02/A08/A09/A16 descriptor/fingerprint/content generation | Consolidates generated contracts and full-input fingerprint | Field/packing equivalence, generation freshness and negative schema tests |
| A03/A04 TS semantic helper parity/mutability | Not automatically solved by deleting UI Views | Preserve relevant content values; clarify their actual supported behavior and immutable ownership |
| A05 style validator | New normalization must use the actual whitespace/NUL rule | Shared TS/Rust examples; no copied overescaped regex |
| A06 type-only ViewSlot leakage | Old authoring slot surface removed at cutover | Emitted declarations contain only restricted new refs/control APIs |
| A07/A10–A14 route/artifact/oracle evidence | No automatic correctness improvement from architecture | Record actual selected addon and route; correct misleading labels; use production checks |
| A15 historical documentation drift | Current source remains descriptive authority | Update guide/inventory; do not restore already removed generic Rust APIs |
| B01 shared Arc→static mutable View runtime | Specific obsolete owner is deleted | New entrypoints have qualified handles, owned decode and safe guards; audit remaining FFI separately |
| B02 unsafe Send/Sync | Not fixed by naming the object OccurrenceDocument | Compiler-checked portable core, actual callback/payload bounds, owner-thread backend; remove unsafe assertions |
| B03/B04 desired mutation followed by flush error | Replaced with explicit prepare/install/ack and separate frame failure | Inject failure before/after acceptance; no ambiguous acknowledgement or replay |
| B05 animation attachment bypass | Animation participates in same final binding validation | Inactive-frame duplicate attachment/disposal tests |
| B06 environment-wide staged abort | Remote UI builder/edit transactions disappear | Closing host A leaves host B's UI/resources/presentation intact |
| B07 preparation-time retirement | Logical invalidation separated from captured product ownership | Delayed receipt with removed controls/content; exact pins and stale-input rejection |
| B08/B09/B12/B13 close/cleanup/restoration | Ownership becomes simpler but physical cleanup still needs verification | Explicit close, startup/output failure, dependency-ordered cleanup and worker lifetime |
| B10/B11 weak/path/style retention | Obsolete UI hint/path/style tables removed | No residual callers/metadata; live versus capacity accounting for remaining caches |
| B14 poisoned control fallback | New route must fault explicitly rather than claim spacer/input success | Native invariant/error reporting tests |
| B15/B16 harness/session lifecycle | Clear acceptance and initialization boundaries specified | Qualify before publishing cached session; harness does not claim rollback after acceptance |
| C01 nested History dependencies | New document identifies changed owner/root frontier | Nested content/state-height test; later Surface has no special live-batch exclusion |
| C02 Source retention/restart | Independent content contract | Preserve full Markdown segment or valid checkpoint rules; do not assume DOM fixes truncation |
| C03/C04 glyph clipping and row parity | Independent physical correctness | Existing full/row-window/wide-glyph regression fixtures through new layout |
| C05 exact ticket failure | Explicit preparation error, never empty successful paint | Missing/poisoned product injection at owning content boundary |
| C06/C07 legacy layout metadata/cache scope | Old general layout removed at M2; M1 remains subject to its limits | M1 parity and M2 dependency/cache tests; no blanket historical defect claim |
| C08 full-Surface terminal lowering | May remain intentionally | Measure physical diff/lowering separately; no unsupported damage-bounded claim |
| C09 partial external output | Not removable by a retained node design | Confirmed-prefix ledger, unknown-suffix state, no automatic scrollback replay |
| C10 editor change-event discrepancy | Explicit new React onEdit/text-only/selection contracts | Preserve native commands; verify and document public event migration |
| C11 idle slot ticks | Composition-only slots removed; real animations have demand deadlines | Static/one-frame/hidden cases have no recurring deadline |
| C12 append grapheme-index cost | Independent performance work | Stage counters and actual workloads; not labelled O(delta) by assumption |
| C13/C14 projection/provenance concerns | Independent semantic-content correctness | Preserve validators/source coordinates; test actual route before declaring a defect |
| C15 input injection versus physical decoder | Independent capability/test-surface distinction | Document injected versus real events; preserve native interpretation |
| C16 queue/backpressure | New finite event contract | Ordered discrete events; typed backpressure, not silent loss |
| C17 large-control limits | Not automatically solved by Taffy | Preserve checked geometry limits; benchmark intended large-edit workloads |
| C18 theme-cache interpretation | Old UI atom cache removed; content/style rules remain | Theme/effort recolor of all resident matching occurrences |
| C19 native wait versus TS environment broker | New driver explicitly services native environment work | Native-only content/tick/presentation test with no JS frame pump |

Do not use this table as authorization to sweep every independent issue into the runtime PR. Touch only the replacement boundary and required invariants; preserve remaining issues with a precise next proof and owner.

---

## 24. Owner approvals and compatibility

The supplied proposal authorizes planning, not implementation. The following are the **recommended approvals**, with no unresolved alternative hidden in them:

| Consequential approval | Recommended choice |
|---|---|
| Public composition change | Adopt React at M1; remove custom View/defineView/state authoring as the canonical path |
| Public ordinary state API | Replace standalone ViewState with occurrence refs for explicit overrides; keep native controllers/resources with independent lifetimes |
| Dependencies | Pin React/reconciler and selected Taffy version; no additional renderer/actor/schema framework |
| UI ABI change | New generated typed UI batch; remove old structural/publication ABI after consumer cutover |
| Literal content integration | Separate React-owned content section of the atomic envelope, sharing existing content implementation; public Source payload FFI remains sole route |
| Legacy staging | Permit one private native renderer adapter at M1; require M2 next and delete it, not a permanent old/new toggle |
| Editor events | Explicit onEdit snapshot event plus text-only/selection conveniences; native key behavior preserved |
| Native event queue | Finite backpressure contract instead of silently dropping discrete events or unbounded buffering |
| Final layout behavior | Iyon Flex/Grid over Taffy with documented terminal rounding; review intentional differences rather than masking them |
| History/Surface | Preserve current physical behavior initially; component-only Surface and explicit export port are separately scheduled changes |

These are approval items because public consumers and dependencies change. They are not architectural questions for the implementation agent to reconsider. Ordinary file placement, generated numeric expansion, Rust borrow splitting and test organization follow the specified contracts.

### Compatibility that is preserved

Preserve content Source identity/coordinates/revision ordering, Source sharing, immutable Funnel semantics, independent Connector execution, transactional switching, native editor/paste/key interpretation, real animation clocks, styling/theme behavior, terminal output/restoration and exact receipt discipline. Maintain generic framework ownership and rebuild the native addon before evaluating integration.

### Compatibility intentionally not promised

The old immutable View authoring API (including View-returning typed-diff/semantic-content helpers), manual ViewState attachment/disposal graph, NativeRef/build/path/lease APIs, source-compatible arbitrary Hanging constructors and generic Rust application authoring do not survive as permanent APIs. Port real behaviors into React composition/common content, not wrappers around the old architecture.

Cross-parent React reparenting is not promised to preserve local control identity. Ordinary same-parent keyed moves are. No browser DOM/CSS/event compatibility, arbitrary native text node or full GUI feature parity is implied.

External application code was not provided for direct inspection in this task. Its public consumer port is a release gate, not an assertion that unseen files have been verified. The in-repository consumer fixture and examples provide immediate implementation targets; the actual application owner must run its behavior trace before removing compatibility support.

---

## 25. Requirements closure and completion checklist

### 25.1 The ten decisions requested by the newer proposal

| DOM proposal decision | Resolution |
|---|---|
| Scope/alternatives | §2 compares the three required approaches and selects direct occurrences with React, not a smaller immutable-View endpoint |
| Frontend timing | §1.3 and T3–T5 bring React forward; no second temporary reconciler |
| State model | §§4, 7: declared/override/native/derived fields; ordinary ViewState owner removed |
| Commit semantics | §§8–9: bounded typed wire, sparse preflight, reservations, infallible apply and preallocated ack |
| Frame isolation | §13: owned exact products, one in-flight frame, no whole-tree snapshots |
| Identity/lifetime | §§4, 6: host/type/generation qualification, single parent, explicit retire, no orphan leases |
| Invalidation | §§15–17: effect domains, necessary ancestors, backend cache, content/environment revisions |
| Native controls/content | §§11–12: retain true native mechanics and Source/Funnel/Connector/Port ownership |
| Sharing | §4.2 and §22.4: share resources; distinct mounted occurrences; measure memory |
| Compatibility | §24: explicit approvals, supported changes and external consumer gate |

### 25.2 V5 constraint cross-check

| V5 requirement | Handoff implementation |
|---|---|
| React canonical, pure speculative render | §10 |
| Three semantic protocols; no generic setProps/fullStyle | §§7–9 |
| Content bypasses React; no per-tick UI transport | §§11, 14, 19, 22 |
| No structural Text/Hanging or renamed equivalents | §§6–7, 11, 17 |
| Base update under override survives clear | §7.4, metadata-only completion §13.4 |
| One desired acceptance before normal React commit returns | §§9–10 |
| Desired versus visible and exact content selection | §§11.4, 13 |
| Source/Funnel/Connector/Port separate responsibilities | §11 |
| Taffy final general layout, specialized measurements | §17 |
| Component-only Surface; completion is not freeze | §18, separately staged |
| Host Environment native-first, relevant subrevisions | §§14–15, 18 |
| JS owns closures, native owns input/subscriptions | §12 |
| No permanent parallel migration path | §§16, 20–21 |
| Measure actual production transport and absolute costs | §22 |

This handoff resolves the **retained UI runtime and its layout cutover**, not every deferred feature in V5 §36. New Source families, a new parser/rate policy, full virtualization, complete terminal capability services and GPUI platform feature implementation remain separate V5 deliverables. They consume the fixed interfaces here and must not alter the core ownership/transport model. Explicit scope is preferable to inventing unresearched implementations for unrelated features.

### 25.3 Final acceptance checklist

- [ ] One canonical React frontend and one native occurrence document; no immutable UI publication layer underneath.
- [ ] Property, topology, literal, binding and native-only scenarios meet exact plane counters.
- [ ] Failed preflight leaves desired state and resource memberships unchanged; acknowledgement cannot become ambiguous after acceptance.
- [ ] In-flight products remain exact; later desired/input/Source work is never promoted by an older receipt.
- [ ] Detach/retire/close have generation-safe, host-local, explainable lifetime; implicit resources clean up and external Sources survive.
- [ ] Controls, content, Unicode, theme, current terminal/History and consumer behavior gates pass.
- [ ] M1 deletes old TS/native publication machinery; M2 deletes the ordinary legacy renderer/general-layout adapter.
- [ ] Generated files, public declarations, tests, benchmarks, inventory and living guide reflect the actual new route.
- [ ] Performance report records actual binaries/profiles, counters, absolute costs and memory; independent risks are not falsely closed.

The implementation is not done merely because a new occurrence type exists. It is done when an ordinary local change can be explained as **one accepted semantic delta to its native owner**, followed only by necessary native consequences, and the superseded publication mechanisms no longer exist.

---

## 26. Evidence register

### 26.1 Supplied specifications

**[DOM]** `DOM-LIKE-RETAINED-RUNTIME-PLANNING(1).md`, supplied with this request, read in full. Key sections: §3 candidate occurrence ownership; §5 V5 constraints; §6 ten required decisions; §7 required deliverable and acceptance; §8 reading map and evidence limits. Baseline in the document is `1e93540`. Its status is a planning proposal, not approval of API changes.

**[V5]** `IYON-UI-PRELIMINARY-DESIGN-v5(2).md`, revision 5 dated 2026-08-31, supplied with this request, read in full. Key sections: §§0–9 destination/React/planes/state; §§10–16 content and ergonomics; §§17–20 Surface/commit; §§21–26 events/environment/layout/native frames; §§31–37 migration/validation/deferred choices/anti-goals. Input hashes are at the beginning of this handoff.

### 26.2 Repository evidence

All repository references below are pinned to **`1e935406c707ad42eb819d259f0a456f0a1129e7`**. Source ranges describe targeted reads, not a claim that every file in this large repository was re-audited. The atlas supplies cross-subsystem historical navigation and explicitly qualified findings. The current commit was verified through the GitHub connector before planning. Relative source ranges in the table were fetched directly; detailed internals attributed to the atlas remain historical evidence unless separately listed as inspected source.

| ID | Inspected source / location | Supports |
|---|---|---|
| R01 | [main commit](https://github.com/alexykn/iyon-tui/commit/1e935406c707ad42eb819d259f0a456f0a1129e7), branch metadata and tree `b2194f90812b8195319c8007d453025b41e94f60` | Actual current baseline and generic Rust authoring cleanup; uploaded V5 blob match |
| R02 | [composition/execution.ts](https://github.com/alexykn/iyon-tui/blob/1e935406c707ad42eb819d259f0a456f0a1129e7/packages/iyon-tui/src/composition/execution.ts#L1-L150) | `ScopeSemanticTable`, `RetainedExecutionScope`, staged props/output/dependencies/publication |
| R03 | [retained-dag.ts](https://github.com/alexykn/iyon-tui/blob/1e935406c707ad42eb819d259f0a456f0a1129e7/packages/iyon-tui/src/transport/structural/retained-dag.ts#L1-L180) | Native hints, NodeId promotion, root leases, scratch/style caches and identity counters |
| R04 | [native view_abi.rs](https://github.com/alexykn/iyon-tui/blob/1e935406c707ad42eb819d259f0a456f0a1129e7/crates/iyon-tui-native/src/tui/view_abi.rs#L1-L175) | Old native View ABI surface, leased/weak references, paged reference slots |
| R05 | [CURRENT-ARCHITECTURE.md](https://github.com/alexykn/iyon-tui/blob/1e935406c707ad42eb819d259f0a456f0a1129e7/docs/architecture/CURRENT-ARCHITECTURE.md#L1-L150) and [atlas inventory](https://github.com/alexykn/iyon-tui/blob/1e935406c707ad42eb819d259f0a456f0a1129e7/docs/architecture/atlas-4355c02/COMPREHENSIVE-REPORT.md), especially §§1–9 | Current ownership map and historical detailed state/content/layout/reference contracts; atlas baseline remains `4355c02` |
| R06 | [application/host.rs](https://github.com/alexykn/iyon-tui/blob/1e935406c707ad42eb819d259f0a456f0a1129e7/crates/iyon-tui/src/application/host.rs#L1-L200) | `HostInner` candidate Options, exact epoch/revision intent, content/state owners, shared control wrappers |
| R07 | [application/kernel.rs](https://github.com/alexykn/iyon-tui/blob/1e935406c707ad42eb819d259f0a456f0a1129e7/crates/iyon-tui/src/application/kernel.rs#L1-L180) | Concrete `NativeRuntime`, native input/output routing, mount-based deferred retirement and invalidation |
| R08 | [application/environment.rs](https://github.com/alexykn/iyon-tui/blob/1e935406c707ad42eb819d259f0a456f0a1129e7/crates/iyon-tui/src/application/environment.rs#L1-L190), [output/event.rs](https://github.com/alexykn/iyon-tui/blob/1e935406c707ad42eb819d259f0a456f0a1129e7/crates/iyon-tui/src/output/event.rs) | Native pending queue/weak hosts/Source owner, unsafe Send/Sync assertions, erased non-Send payload boundary |
| R09 | [application/content.rs](https://github.com/alexykn/iyon-tui/blob/1e935406c707ad42eb819d259f0a456f0a1129e7/crates/iyon-tui/src/application/content.rs#L1-L190); Source storage location [source_store.rs](https://github.com/alexykn/iyon-tui/blob/1e935406c707ad42eb819d259f0a456f0a1129e7/crates/iyon-tui/src/application/source_store.rs), detailed in atlas §7 | Actual Block/Stream and Funnel families, annotation record, immutable Source snapshot and lineage; storage is retained rather than redesigned here |
| R10 | [tools/tui-abi](https://github.com/alexykn/iyon-tui/tree/1e935406c707ad42eb819d259f0a456f0a1129e7/tools/tui-abi), [tui-abi-gen](https://github.com/alexykn/iyon-tui/tree/1e935406c707ad42eb819d259f0a456f0a1129e7/tools/tui-abi-gen) and findings A01/A02/A08/A09/A16 | Existing schema/generator location; explicit proposed consolidation, not an assertion that content packing is already generated |
| R11 | [ISSUES.md](https://github.com/alexykn/iyon-tui/blob/1e935406c707ad42eb819d259f0a456f0a1129e7/docs/architecture/atlas-4355c02/ISSUES.md) including interpretation and grouped disposition | Exact issue IDs, confidence, source-confirmed behavior versus open reachability/contract questions |
| R12 | [root package.json](https://github.com/alexykn/iyon-tui/blob/1e935406c707ad42eb819d259f0a456f0a1129e7/package.json), [package manifest](https://github.com/alexykn/iyon-tui/blob/1e935406c707ad42eb819d259f0a456f0a1129e7/packages/iyon-tui/package.json), [core Cargo.toml](https://github.com/alexykn/iyon-tui/blob/1e935406c707ad42eb819d259f0a456f0a1129e7/crates/iyon-tui/Cargo.toml) | Real commands, Bun 1.4.0, package exports, existing proptest/insta/native test features and no current Taffy dependency |
| R13 | [AGENT_VALIDATION.md](https://github.com/alexykn/iyon-tui/blob/1e935406c707ad42eb819d259f0a456f0a1129e7/.github/AGENT_VALIDATION.md) | Existing fast agent branch and full PR validation; native staging and platform coverage |
| R14 | [presentation/mod.rs](https://github.com/alexykn/iyon-tui/blob/1e935406c707ad42eb819d259f0a456f0a1129e7/crates/iyon-tui/src/presentation/mod.rs#L1-L180), [retained_state/mod.rs](https://github.com/alexykn/iyon-tui/blob/1e935406c707ad42eb819d259f0a456f0a1129e7/crates/iyon-tui/src/retained_state/mod.rs), [capture.rs](https://github.com/alexykn/iyon-tui/blob/1e935406c707ad42eb819d259f0a456f0a1129e7/crates/iyon-tui/src/retained_state/capture.rs#L1-L155) | Existing private factories, state module separation and `StateFrameView` immutable read seam for M1 adaptation |
| R15 | [AGENTS.md](https://github.com/alexykn/iyon-tui/blob/1e935406c707ad42eb819d259f0a456f0a1129e7/AGENTS.md) | Generic framework ownership, approval boundaries, validation-at-ingress and maintenance discipline; historical authoring names are not restoration requirements |

### 26.3 External primary-source research

External systems support particular implementation lessons; they do not override either supplied Iyon specification.

| ID | Primary source | Lesson and limit |
|---|---|---|
| X01 | [React reconciler README, v19.2.0](https://github.com/react/react/blob/v19.2.0/packages/react-reconciler/README.md) | Mutation versus persistence, speculative creation, host lifecycle/subtree cleanup; experimental renderer API, not a stable host ABI |
| X02 | [React v19.2.8 reconciler package](https://github.com/react/react/blob/v19.2.8/packages/react-reconciler/package.json), [custom host shim](https://github.com/react/react/blob/v19.2.8/packages/react-reconciler/src/forks/ReactFiberConfig.custom.js#L45-L185), [React state identity](https://react.dev/learn/preserving-and-resetting-state) | Selected source version and actual mutation/priority hooks; frontend lifecycle remains Fiber-owned. Installed npm artifacts still require build qualification |
| X03 | [WHATWG DOM Standard](https://dom.spec.whatwg.org/#concept-node-pre-insert) | Ownership/insert/move analogy and anchor/ancestor validation; not atomic Iyon acceptance or browser compatibility |
| X04 | [Inside Flutter](https://docs.flutter.dev/resources/inside-flutter) | Composition and derived rendering may use separate representations; dependency-based layout, not another reconciler for Iyon |
| X05 | [Taffy crate documentation](https://docs.rs/taffy/latest/taffy/), [v0.12.2 Cargo.toml](https://github.com/DioxusLabs/taffy/blob/v0.12.2/Cargo.toml), [tagged tree implementation](https://github.com/DioxusLabs/taffy/blob/v0.12.2/src/tree/taffy_tree.rs#L120-L235) | High-level tree storage/cache integration and selected features. Tagged source, not a moving latest page, governs the implementation pin |
| X06 | [GPUiX repository documentation](https://github.com/remorses/gpuix) | Direct React mutation→retained native tree and callback ownership comparison; its generic transport/timing claims are not Iyon contracts or measurements |
| X07 | [Node-API typed arrays/ArrayBuffers](https://nodejs.org/api/n-api.html#napi_get_typedarray_info) | Buffer ownership and native qualification boundary; not an assertion that Bun 1.4.0 implements every current Node-API shared-buffer detail identically |
| X08 | [Rust reference: undefined behavior](https://doc.rust-lang.org/reference/behavior-considered-undefined.html) | Aliasing, invalid references and data races remain obligations inside unsafe code; ownership/thread placement must establish safety |
| X09 | [CommonMark 0.31.2 link reference definitions](https://spec.commonmark.org/0.31.2/#link-reference-definitions) | Later definitions can affect earlier references; a fixed stable-tail heuristic is not a complete general-Markdown correctness argument |
| X10 | [CSS Scroll Anchoring draft](https://www.w3.org/TR/css-scroll-anchoring-1/#scroll-adjustment) | Stable identity plus offset adjustment; use only the principle, not an HTML/CSS compatibility claim |

### 26.4 Evidence limits and reproducibility

Research used the supplied specifications, the pinned GitHub source/metadata and external primary documentation. This is a targeted architecture audit, not a claim to have executed Iyon, re-read every implementation file, formally proved the new design, or benchmarked a runtime that does not yet exist.

The Python model and result file are reproducible design checks of the selected algorithms. Their finite coverage and separate scope are documented in §22.6. No evidence limitation changes the selected design into a menu: implementation decisions are fixed, while empirical acceptance remains the job of the explicit tranche gates.

**Final instruction to the implementation agent:** implement the direct ownership model, then delete the old publication model. Do not preserve the wrong destination merely because it makes the first compatibility patch easier.
