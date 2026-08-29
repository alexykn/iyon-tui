<!--
Generated replacement handoff.
Generated UTC: 2026-08-29T18:04:49.592568+00:00
Original SHA-256: 62c16f30718e314f8b61b78cd4ba489fc58f7a9d59615a1f707de77ca712f4bf
-->

# PERF-13 — Three-Plane Runtime
## Resolved implementation handoff

**Status:** implementation-ready architecture  
**Scope:** TypeScript API/runtime, structural transport, Rust host/runtime, retained state, retained content, content FFI, migration, and deletion of superseded high-volume paths  
**Incoming architecture:** API-H3 composition/transport seam  
**Authority:** this resolved section supersedes every conflicting or tentative statement in the integrated baseline body that follows it

> PERF-13 stops the retained semantic `View` DAG from being the universal carrier for structure, mutable state, and high-volume content. It establishes three separate planes that meet only through explicit identities and the host frame transaction.

```text
STRUCTURAL PLANE
retained, backend-neutral semantic View DAG

RETAINED STATE PLANE
host-owned mutable occurrence state

CONTENT PLANE
environment-owned Sources plus host-owned Ports and Connectors
```

The content connector model in PERF-13 is **cold**. Buffered/hot fan-out, arbitration, background producer threads, and speculative preprojection are not part of this change.

---

## 0. How to use this handoff

### 0.1 Normative language

`MUST`, `MUST NOT`, `SHOULD`, and `MAY` are used in their usual design-specification sense.

The document has two parts:

1. **Resolved architecture and delivery plan.** This part closes the remaining cross-plane seams and is authoritative when any older wording conflicts with it.
2. **Integrated baseline specification.** This is the complete earlier PERF-13 handoff, retained so that no already-settled detail is lost. It remains normative for topics not explicitly superseded here.

The implementation agent is not expected to reopen the decisions in this part. Internal layout, exact Rust record shapes, scratch-buffer sizes, COW strategy, module splitting, and similar representation choices remain implementation freedoms unless an invariant below constrains them.

### 0.2 Baselines

The generated edition records the exact API-H3 branch commit and repository audit in the appendices. That commit, rather than any stale `api-h1` reference in older prose, is the implementation baseline.

The attached PERF-13 handoff remains the source for already-settled high-level architecture:

- three planes;
- opaque occurrence attachments;
- Rust-side effect classification;
- retained Source storage;
- cold Connector scheduling;
- transactional Connector switching;
- native pending/committed epochs;
- one atomic logical frame commit;
- mandatory direct FFI for high-volume content payloads;
- eventual deletion of the superseded high-volume runtime path.

### 0.3 Decision summary

| Area | Final decision |
|---|---|
| H3 publication vs frame commit | H3 commits the **desired structural revision**. PERF-13 later commits the **visible frame revision**. A failed frame never rolls back an accepted desired revision. |
| Structural failure placement | Every deterministic, permanently failing structural/attachment check runs in H3 prepare. Frame preparation does not rediscover ordinary attachment errors. |
| Semantic attachment identity | Semantic nodes carry branded framework `HandleId` values only. Structural lowering resolves them to native generational IDs through a plane-neutral resolver. |
| Resource resolver | One runtime-level `NativeResourceRegistry`/resolver is shared by structural, state, content, and component transports. Structural transport does not import state or content transport. |
| Source ownership | Sources are owned by the native **environment**, not a `TuiHost`. ViewState, ContentPort, and Connector are host-owned. |
| Multi-host wake | One environment wake broker drains all pending live hosts. Native host epochs and the pending-host set are authoritative; a single edge-triggered JS microtask is the normal wake hint. |
| Automatic flush errors | Automatic drains never throw out of a microtask. They record a structured host error, report it through the runtime error channel, and leave retryable work pending. Explicit barriers throw/reject deterministically. |
| Frame ordering | Connector activation, content projection, measurement, and placement participate in one convergence loop. Activation is not a post-layout phase. |
| Stateful occurrence | Every physical, state-capable occurrence owns a base `OccurrenceBox`, even when visually undecorated. Mutable borders/padding never create a new structural wrapper. |
| `Decorated` migration | Property-only `Decorated` wrappers are compatibility input normalized into the owner occurrence's base box. Semantically distinct wrappers remain until separately proven equivalent. |
| ViewState typing | TypeScript types property/value shape. An exhaustive Rust node-kind capability table validates attachment and mounted mutation. ViewState is not generic over node kind. |
| `null` vs clear | `null` is a semantic value only for explicitly nullable fields. Clear APIs remove an override and reveal the View's base value. `undefined` means “not present in this patch.” |
| Port unmount/remount | A port retains Connector membership and requested selection while unmounted. It is cold and has no projection until a visible mount commits. |
| Disposal | Individual disposal is explicit and non-cascading. A mounted/in-use resource rejects disposal. Host/environment teardown is the documented owner-death cascade. |
| Public content nouns | Canonical API is `port.connect(source, funnel)`. A Funnel is immutable and Source-neutral. A Source-bound intermediate is not called a Funnel. |
| Host affinity | Attaching a host-owned ViewState or ContentPort makes that View subtree host-affine. Publishing it to another host is an H3 prepare error. |
| Annotation identity | Source annotations contain semantic, environment-level style data, never host-native Style IDs. They are resolved for the target host/theme during projection. |
| Viewport ownership | ScrollPane/RowViewport owns offset and follow state. ContentPort owns allocation/clip binding only. Connector receives a read-only viewport context. |
| FFI ordering | Native Source mutex serialization and the assigned native Source revision are the linearization order. There is no caller-supplied sequence token in ABI v1. |
| Native artifact | `bun:ffi` loads the same staged `.node` artifact as the Node-API control surface. One generated locator and one ABI metadata probe are mandatory. |
| OOM | Process/runtime-fatal under the ordinary Rust allocator. `OUT_OF_MEMORY` is not a recoverable ABI status. User-controlled size limits fail before allocation with explicit limit errors. |
| Legacy stream paths | Algorithms and immutable snapshot types may survive internally; no second public/runtime high-volume mutation, scheduling, identity, or transport architecture may survive PERF-13 completion. |
| Delivery | Eight stacked tranches, each with stop gates and tightly bounded temporary dual-path allowances. |

---

## 1. Architectural north star and non-negotiable invariants

### 1.1 The three planes

#### Structural plane

The structural plane carries only retained semantic structure and structural attachment identity:

- semantic node kind and immutable structural parameters;
- children and component structure;
- backend-neutral attachment `HandleId` values;
- other information whose change creates a new semantic View value.

It does **not** carry mutable geometry patches, presentation patches, Source bytes, Source snapshots, Connector control mutations, native slots, or host-native Style IDs.

#### Retained state plane

The retained state plane carries mutable state attached to one physical occurrence at a time:

- geometry overrides;
- presentation overrides;
- interaction-driven visual state where already in PERF-13 scope;
- native effect classification and dirty propagation metadata.

A state mutation does not create a new semantic View value. The attachment identity itself remains structural.

#### Content plane

The content plane carries retained mutable Sources, immutable Funnel specifications, host-owned ContentPorts, and host-owned Connectors. Source payload mutation does not rebuild the semantic View DAG.

The structural plane says only:

```text
this occurrence attaches ContentPort HandleId P
```

It does not dispatch Source operations or Connector control.

### 1.2 Cross-plane rules

1. A semantic View may contain a backend-neutral attachment handle, never a native resource ID.
2. A transport may resolve a handle through the plane-neutral native-resource resolver; it may not call another plane's dispatcher.
3. Only the host frame transaction combines desired structure, retained state, Connector control, Source snapshots, layout, projection, damage, and paint.
4. Native state is authoritative for pending work, liveness, binding, Source revision, Connector activation, and the visible frame.
5. TypeScript provides typed ergonomic APIs and wake hints. It does not mirror the native subscription graph or classify native invalidation effects.
6. Failed candidate work cannot partially mutate the visible frame.
7. No public API may expose an occurrence path or use semantic `NodeId` as an occurrence address.

### 1.3 Cold Connector invariant

For each mounted ContentPort:

- at most one Connector is the committed visible Connector;
- at most one different Connector is the requested activation candidate;
- inactive Connectors retain identity and configuration but hold no projection and do not subscribe their host for Source wakeups;
- a failed switch leaves the previous committed Connector and projection visible;
- an unmounted port has no committed projection and all of its Connectors are cold.

### 1.4 No hidden second architecture

Compatibility layers are allowed only when they terminate immediately in the new architecture. A compatibility method may translate arguments and invoke `Source.replace`, for example. It may not preserve an old host queue, old native stream registry, old payload bridge, old content scheduler, or old repaint path.

---

## 2. Vocabulary, identities, and ownership

### 2.1 Environment

An **environment** is one live native runtime associated with one JavaScript realm/module instance. It owns resources that may be shared across Tui hosts in that realm.

The environment owns:

- the Source registry;
- the live-host registry;
- the environment pending-host set and wake latch;
- the loaded FFI library handle and ABI metadata;
- environment-level semantic style interning, if interning is used;
- fatal/poison state shared by its native entry surfaces.

A Source may outlive any individual host, but it may not cross environments. Cross-worker or cross-realm Source transfer is not supported by PERF-13.

### 2.2 Tui host

A **TuiHost** owns frame-visible state and resources whose meaning depends on one layout/rendering host:

- desired structural revision;
- committed visible frame;
- pending and committed host epochs;
- ViewState registry;
- ContentPort registry;
- Connector registry;
- viewport/controller state;
- host runtime error channel;
- backend surface state and damage history.

### 2.3 Identity table

| Identity | Domain | Owner | Meaning |
|---|---|---|---|
| semantic `NodeId` | semantic DAG | framework | Stable semantic-node identity. Never an occurrence address. |
| `HandleId` | framework/native-resource seam | environment registry | Backend-neutral semantic reference to a registered resource. Branded by resource kind. |
| environment slot + generation | native process | native runtime | One live JS/native environment generation. |
| host slot + generation | environment | environment | One live TuiHost generation. |
| `ViewStateId` | host | TuiHost | Native retained state resource. |
| `ContentPortId` | host | TuiHost | Native retained mount point. |
| `ConnectorId` | host | TuiHost | Native Source/Funnel/Port link and control state. |
| `SourceId` | environment | environment | Native retained Source storage and revision stream. |
| Funnel value/ID | value/environment | immutable | Source-neutral transformation/configuration. A compiled native form may be interned. |
| occurrence key | one desired/visible host tree | host | Native physical occurrence identity. Not public and not encoded in semantic Views. |
| desired structural revision | host | host publication controller | Latest structurally accepted root. |
| visible frame revision | host | frame transaction | Latest complete logical frame committed to the host. |
| host pending/committed epoch | host | scheduler | Whether work accepted by any plane still needs a frame attempt. |
| Source revision | Source | Source mutex | Native linearization order for Source mutations. |

### 2.4 Generational native handles

Every native registry ID is generational. A slot reuse must never make an old JavaScript wrapper valid again. FFI Source identity is therefore:

```text
environment_slot
environment_generation
source_slot
source_generation
```

Host-owned control APIs additionally validate host slot/generation. Older wording that called the first pair `runtime_slot/runtime_generation` is corrected: for Source calls it denotes the **environment**, not a Tui host.

### 2.5 Binding is dual during a transition

A host-owned attachment can temporarily have:

- a **desired binding**, established by H3 commit; and
- a different **visible binding**, retained by the old committed frame until the new frame commits.

This is not duplicate mounting. It is one transactional remount in progress. Duplicate-use validation concerns two occurrences in the same desired candidate.

The registry must distinguish desired and visible binding so that:

- mutations after structural publication validate against the desired target kind;
- the old frame remains usable during retry;
- disposal cannot race a still-visible resource;
- a successful frame commit can swap bindings without a gap.

### 2.6 Ownership and disposal principles

Individual resources do not silently cascade:

- `Source.dispose()` with live Connectors fails with `SOURCE_IN_USE`;
- `ContentPort.dispose()` while desired-bound or visible-bound fails with `PORT_MOUNTED`;
- `ContentPort.dispose()` with Connectors fails with `PORT_IN_USE`;
- equivalent mounted/in-use checks apply to ViewState and Connector operations defined later.

Owner death is different:

- disposing a TuiHost forcibly unmounts its visible frame, removes all Source subscriptions, and invalidates its ViewStates, Ports, and Connectors;
- disposing the environment first tears down hosts and then releases Sources;
- Sources survive ordinary host disposal when the environment remains alive.

JavaScript finalizers are leak-recovery aids only. They do not define semantic disposal timing.

---

## 3. The API-H3 publication seam

### 3.1 Two authoritative states, not one

PERF-13 formally distinguishes:

```text
DESIRED STRUCTURAL STATE
latest H3 publication accepted by the host

VISIBLE COMMITTED FRAME STATE
latest structure + retained state + content + layout + paint
that completed the host frame transaction
```

This distinction resolves the apparent conflict between H3's infallible publication commit and PERF-13's later fallible frame preparation.

H3 commit means:

> the semantic composition and host agree that this is now the desired structural root.

It does not, by itself, mean:

> terminal output for this root has already been prepared and committed.

### 3.2 H3 prepare responsibilities

H3 prepare must perform **all ordinary deterministic work that can permanently reject the structural candidate**. For PERF-13 attachments this includes:

1. materializing the complete semantic candidate required by H3;
2. deriving host affinity for the candidate DAG/subtree;
3. resolving every ViewState and ContentPort `HandleId` through the plane-neutral resolver;
4. validating resource kind, environment, host owner, generation, liveness, and disposal state;
5. expanding occurrence use sufficiently to detect duplicate attachment use caused by DAG reuse;
6. rejecting a ViewState used by more than one candidate occurrence;
7. rejecting a ContentPort used by more than one candidate occurrence;
8. validating that each attachment is legal on the target semantic/native node kind;
9. validating every retained ViewState override against the target node's capability set;
10. validating ContentPort host-node compatibility;
11. validating incompatible host affinities within a reused/composed View;
12. allocating/preparing every record needed so commit performs no ordinary allocation;
13. acquiring short-lived prepared-publication leases for all resolved resources.

A duplicate-use diagnostic should name both semantic occurrence paths for debugging, but those paths are not stable public identities.

These checks must not be deferred to the frame transaction. If they fail, H3 aborts and both the old composition publication and old desired host root remain authoritative.

### 3.3 H3 commit responsibilities

H3 commit is an infallible authoritative state transition. It must consist only of prevalidated, preallocated operations:

1. install the prepared candidate as the host's latest desired structural revision;
2. update H3 composition bookkeeping/`projectedOutput` using commit-only assignments;
3. establish desired attachment bindings from the prepared use table;
4. release the superseded desired candidate's leases where they are no longer needed;
5. increment the host pending epoch;
6. put the host in the environment pending-host set;
7. return the publication receipt and wake disposition.

No user callback, Funnel projection, layout, terminal I/O, allocation that can normally fail, resource lookup, or property compatibility check occurs in this phase.

A prepared token pins the host and resources across prepare/commit. Host disposal cannot interleave between those calls; the controller must serialize disposal with prepared publications. The same serialization covers capability-relevant ViewState mutation and Port disposal. In v1, synchronous prepare/commit on the JS thread naturally gives this exclusion; the native lease/host controller must preserve it if future worker entry is added. A prepared state attachment either blocks an incompatible override mutation until commit/abort or makes that mutation validate against the provisional target, so commit never discovers a newly incompatible mask.

### 3.4 H3 abort responsibilities

Abort releases prepared leases and scratch storage. It changes neither:

- H3's current projected output;
- the host's desired root;
- desired attachment bindings;
- the visible frame;
- committed Connector state.

### 3.5 What frame failure means after H3 commit

After a desired revision is committed, a later frame attempt may still fail because of a retryable backend/preparation condition or a fatal internal invariant. In that case:

- the new desired root remains authoritative;
- the previous visible frame remains the host's logical committed frame;
- `pending_epoch != committed_epoch` remains true;
- the failure is recorded through the host runtime error channel;
- a later valid wake or explicit barrier retries the latest desired root;
- composition is not rolled back to match the old visible frame.

This is the intended state, not a split-brain bug: semantic desired state and visible state have explicit, separately named revisions.

### 3.6 Superseding a desired revision before visibility

If desired revision B has not become visible and H3 commits revision C:

- C becomes the desired root;
- B may be discarded after its leases are released;
- the old visible frame A remains until a frame for C succeeds;
- the host need not render B merely because it was once desired;
- barriers use host epochs/publication receipts, not an assumption that every intermediate desired revision will be visible.

The normal policy is latest-wins coalescing. Debug counters retain the number of skipped desired revisions.

### 3.7 Publication receipts and visibility barriers

Internally, every H3 commit yields:

```ts
type PublicationReceipt = {
  readonly host: HostHandle;
  readonly desiredRevision: bigint;
  readonly acceptedEpoch: bigint;
};
```

The public API need not expose this exact object, but the runtime must preserve the information.

The semantic rule is:

- ordinary publication returns synchronously when the desired revision is accepted;
- `tui.flush(): void` is the synchronous visibility/error barrier through the host epoch captured at its entry;
- any API that promises committed geometry, screen content, or visible Connector status must first perform the same barrier;
- a compatibility API that historically promised synchronous visibility may call the barrier before returning, without weakening the internal distinction.

A barrier succeeds when all work through its captured epoch has either committed or been superseded by a later successfully committed desired/frame state that semantically includes it. It throws the stored structured failure if it cannot complete.

### 3.8 Frame-time attachment reconciliation is non-failing

The frame transaction still reconciles desired attachment bindings with the candidate occurrence tree, but ordinary validity is already proven. Reconciliation may only encounter:

- a runtime invariant violation;
- a host/environment poison condition;
- process-fatal allocation failure;
- a deliberately injected test failure.

It must not report ordinary duplicate, stale-handle, wrong-host, or unsupported-property errors. Those belong to H3 prepare.

---

## 4. Backend-neutral attachment identity

### 4.1 Semantic node shape

The backend-neutral semantic representation carries only branded framework handles:

```ts
type SemanticViewNode = {
  // existing semantic fields
  readonly stateAttachment?: ViewStateHandleId;
  readonly contentAttachment?: ContentPortHandleId;
};
```

The exact field placement may follow the H3 schema conventions, but the semantics are fixed:

- a `HandleId` is not a native slot;
- a `HandleId` does not expose a backend generation;
- changing the attached handle is a structural semantic change;
- mutating the resource behind the same handle is not structural;
- no native pointer or native Style ID enters the semantic DAG.

A semantic View object must retain a strong JavaScript attachment reference, directly or through an internal immutable attachment record, so GC cannot make a still-reachable View contain a dangling framework handle.

### 4.2 Plane-neutral native-resource registry

The runtime owns one resolver boundary conceptually shaped as:

```ts
interface NativeResourceResolver {
  register(resource: RegisteredNativeResource): HandleId;
  prepareResolve(
    handle: HandleId,
    expectedKind: NativeResourceKind,
    targetEnvironment: EnvironmentHandle,
    targetHost?: HostHandle,
  ): PreparedResourceLease;
  release(handle: HandleId): void;
}
```

Its records include:

- branded resource kind;
- owning environment;
- owning host when host-bound;
- native generational ID;
- live/disposal state;
- prepared/desired/visible lease counts or equivalent safe ownership;
- debug creation information in development builds.

The structural transport depends on this interface, not on `transport/state` or `transport/content`.

```text
state/content/component creation
            │ register/release
            ▼
runtime/native-resource-registry
            ▲ prepareResolve
            │
structural transport   state transport   content control transport
```

### 4.3 Structural lowering

Structural transport lowering occurs in two steps:

1. Lower the backend-neutral semantic candidate while collecting attachment `HandleId` uses.
2. Resolve uses through the registry into prepared native resource leases and encode native `ViewStateId`/`ContentPortId` values only in the backend-specific prepared publication.

The native IDs may appear in native prepared records. They may not be written back into SemanticViewNode or retained as semantic data.

### 4.4 Duplicate attachment semantics

Within one desired candidate:

- a ViewState may occur at most once;
- a ContentPort may occur at most once;
- the same occurrence may attach one ViewState and one ContentPort when its node kind supports both;
- sharing an attached semantic subtree through the DAG and thereby expanding it into two occurrences is a duplicate-use error;
- a candidate use plus the same resource's old visible use is an allowed transition, not a duplicate.

### 4.5 Host affinity

An unattached ordinary View remains host-neutral and reusable across Tui hosts.

Attaching a Tui-owned resource gives the node and every containing semantic value a derived host affinity:

```text
View.container(...).state(tuiA.viewState())  -> affine to tuiA
View.content(tuiA.contentPort(...))          -> affine to tuiA
```

Rules:

- publishing the value to `tuiA` is valid;
- publishing it to `tuiB` is an H3 prepare error;
- composing attachments from two different hosts into one candidate is an H3 prepare error;
- Source and Funnel values do not make a View host-affine because they are not embedded in the View; the host-owned ContentPort is the structural attachment;
- remounting within the same host is valid subject to single-occurrence use and capability checks.

### 4.6 Resource creation and release

`tui.viewState()` and `tui.contentPort()` create the native host resource first, then atomically register the framework handle. A partially created resource is cleaned up before an exception returns to user code.

Release is the inverse:

1. synchronously validate that disposal is legal;
2. remove or mark the framework handle disposing so no new prepare can acquire it;
3. complete the native release/transaction;
4. invalidate the wrapper generation and registry entry.

The registry is infrastructure, not a fourth semantic plane.

---
## 5. Host epochs, environment wake broker, and automatic errors

### 5.1 Native work state is authoritative

Each TuiHost owns monotonically increasing counters:

```text
pending_epoch
committed_epoch
```

Every accepted mutation that can affect a future frame increments or advances `pending_epoch`. A host needs work exactly when:

```text
pending_epoch != committed_epoch
```

This includes accepted structural publication, retained-state mutation, Connector control, Source mutation affecting an active/activation-pending Connector, viewport mutation, and backend invalidation.

The TypeScript scheduler is an edge-triggered wake mechanism. It is never the source of truth and it never owns a mirrored dirty flag whose loss can make native work disappear.

### 5.2 One broker per environment

The environment owns:

- a weak/generational set of pending host IDs;
- an atomic/native wake latch;
- a fair cursor for selecting hosts;
- one TypeScript `EnvironmentWakeBroker` with at most one queued microtask;
- an optional future native-thread event-loop notifier behind the same interface.

A Source mutation that affects Tui A, B, and C performs the following native operation:

1. commit the Source mutation and assign its Source revision;
2. copy the relevant weak subscriber host IDs while holding the Source lock;
3. release the Source lock;
4. advance each still-live host's pending epoch and insert it into the environment pending set;
5. edge-trigger the environment wake latch;
6. return one `schedule_environment_drain` bit to the FFI wrapper.

It does **not** return an unbounded host list to JavaScript and JavaScript does **not** duplicate Source-to-host subscriptions.

### 5.3 Subscriber eligibility

A Source tracks weak subscriptions only for Connectors that can legitimately affect a mounted candidate or visible frame:

- the currently committed Connector of a visible mounted port;
- a requested activation candidate for a desired/visible mounted port;
- any short-lived candidate lease required while a frame attempt is in progress.

Idle Connectors and selected Connectors on a fully unmounted port are cold and do not wake a host.

Weak records include host and Connector generations. Stale records are ignored and opportunistically removed.

### 5.4 Edge-trigger protocol

The wake protocol must be race-safe even when future native producers are added.

Conceptually:

```rust
fn mark_host_pending(host: HostId) -> WakeDisposition {
    host.advance_pending_epoch();
    environment.pending_hosts.insert(host);

    if environment.wake_latched.compare_exchange(false, true).is_ok() {
        WakeDisposition::ScheduleEnvironmentDrain
    } else {
        WakeDisposition::AlreadyScheduled
    }
}
```

The JavaScript wrapper queues one microtask only for `ScheduleEnvironmentDrain`:

```ts
function requestEnvironmentDrain(env: EnvironmentRuntime): void {
  if (env.microtaskQueued) return;
  env.microtaskQueued = true;

  queueMicrotask(() => {
    env.microtaskQueued = false;
    const report = env.native.flushPendingHosts(env.flushBudget);
    env.errorChannel.accept(report.errors);

    if (report.rearm) requestEnvironmentDrain(env);
  });
}
```

The production implementation must catch boundary exceptions and route them to the error channel; the illustrative code omits that boilerplate.

Native `flushPendingHosts` owns latch completion:

1. keep the native wake latch set while taking and attempting a fair batch;
2. remove successfully committed hosts whose epochs are caught up;
3. mark retry-blocked hosts as blocked rather than immediately runnable;
4. clear the latch only after rechecking the pending set;
5. return `rearm = true` if runnable work appeared during the clear/recheck window or the fairness budget was exhausted;
6. allow a concurrent mutation that observes a cleared latch to win the next edge and queue a new microtask.

This closes the lost-wake race without polling.

### 5.5 No microtask retry spin

A retryable frame preparation failure leaves the host semantically pending, but it must not create an infinite microtask loop.

The scheduler records the attempted host epoch and places the host in a retry-blocked state. It becomes runnable again when one of these occurs:

- a new independent mutation advances its pending epoch;
- the failed backend condition explicitly signals readiness;
- user code calls an explicit barrier/retry API;
- a bounded timer/backoff policy is deliberately configured for that failure class.

Merely observing `pending_epoch != committed_epoch` after the same failed attempt does not immediately queue another microtask.

### 5.6 Future native producers

PERF-13 v1 assumes high-volume Source mutation enters through JavaScript/Bun FFI. A future native producer thread must use the same `mark_host_pending` path plus one environment-level event-loop notifier, such as a Node-API thread-safe function or equivalent runtime-supported primitive.

It must not introduce per-Source JavaScript callbacks or a second scheduler.

### 5.7 Error classes

| Failure class | Detection | Delivery | Frame effect |
|---|---|---|---|
| caller validation | API wrapper or native entry validation | synchronous throw/rejection from the initiating call | no mutation accepted |
| structural prepare | H3 prepare | publication rejection | old desired and visible state remain |
| Connector operational | projection/parser/Source-specific Connector preparation | committed Connector status and optional Connector event | old Connector remains visible; unrelated frame work may commit |
| retryable automatic-frame/backend preparation | frame transaction | stored host runtime error; runtime reporter callback; explicit barrier throws/rejects | desired state remains pending; old logical frame remains |
| internal invariant/convergence violation | native frame/runtime | poison host/environment, structured fatal report, subsequent calls fail | no partial logical commit |
| allocation exhaustion | allocator/process | fatal according to build/runtime policy | no recoverable ABI status is claimed |

### 5.8 Automatic drains never throw into nowhere

An automatic microtask drain must not let an ordinary application-visible error escape the microtask callback.

For every failed host attempt, native returns a structured record containing at least:

```ts
type RuntimeFrameErrorRecord = {
  readonly host: HostHandle;
  readonly attemptedEpoch: bigint;
  readonly desiredRevision: bigint;
  readonly phase: FramePhase;
  readonly code: RuntimeFrameErrorCode;
  readonly retryable: boolean;
  readonly diagnostic: string;
};
```

The TypeScript runtime:

1. stores the latest unacknowledged error on the affected host;
2. invokes an optional `tui.onRuntimeError` listener outside native code;
3. invokes the environment's configured reporter when no listener consumes it (`globalThis.reportError` where available, otherwise the package's diagnostic logger);
4. catches listener exceptions and reports those separately;
5. does not clear pending native work merely because the error was reported.

### 5.9 Explicit barriers are deterministic error boundaries

`tui.flush()` and every committed-state readback operation:

1. capture the host pending epoch at entry;
2. synchronously attempt work through that epoch;
3. throw the relevant structured error if completion is impossible;
4. mark that error observed for duplicate-report suppression;
5. return only when committed state satisfies the barrier.

Automatic reporting therefore gives visibility without making application recovery depend on an unhandled microtask exception.

### 5.10 Poisoning and panic containment

An internal invariant failure poisons at least the affected host; an environment-wide registry/ABI invariant poisons the environment. Once poisoned:

- automatic drains stop attempting affected resources;
- the fatal diagnostic is delivered once through the configured reporter;
- every subsequent public operation touching the poisoned scope returns the same `RUNTIME_POISONED` family of error;
- tests may configure panic/abort after diagnostic capture;
- Rust unwinding must never cross a C ABI or Node-API boundary.

OOM remains outside this recoverable mechanism.

---

## 6. Complete frame transaction and convergence

### 6.1 Candidate/commit principle

All mutable frame products are prepared in candidate storage. Until the final logical commit, the host's visible records remain untouched:

- visible occurrence tree;
- visible attachment bindings;
- effective geometry and placement;
- committed Connector selection and projection;
- Source revisions represented by that projection;
- damage baseline and screen model;
- committed host epoch.

Implementation may use scratch records, copy-on-write, journaled deltas, or another representation as long as abort is complete and commit is non-failing after backend application succeeds.

### 6.2 Normative frame algorithm

For one host attempt:

```text
A. Capture attempt boundary
   - target pending epoch
   - latest desired structural revision at/before that boundary
   - relevant state/control queue boundaries

B. Materialize candidate occurrence state
   - start from reusable visible data where valid
   - apply latest desired structure
   - reconcile already-validated attachment leases
   - establish candidate desired/visible transition records

C. Apply retained-state mutations
   - coalesce by ViewState/property where legal
   - compute candidate effective values from View base + overrides
   - classify effects in Rust
   - seed projection/measure/place/paint dirty work

D. Apply Connector/viewport control
   - latest requested selection per port wins within the boundary
   - preserve current committed Connector as fallback
   - apply scroll/follow state to the viewport controller

E. Acquire immutable Source snapshots
   - snapshot only current visible or requested candidate Connectors
   - share one Source revision snapshot among Connectors where possible
   - hold no Source mutex during projection/layout

F. Run the unified convergence loop
   - derive constraints and offered widths
   - project content whose Source/Funnel/input width/theme/viewport key changed
   - handle candidate activation failure with old-Connector/empty fallback
   - measure dirty leaves/subtrees
   - propagate measurements
   - place dirty roots
   - derive final viewport windows/materialization
   - repeat only when placement/width invalidates a projection or measurement key

G. Compute damage
   - compare old and candidate geometry/presentation/content
   - include exposed/moved/removed regions
   - escalate to larger-region/full repaint under benchmark-tuned thresholds

H. Paint a candidate screen/diff
   - use candidate projection and host-resolved styles
   - do not mutate visible frame records

I. Apply the backend update
   - use the backend's buffered/diff protocol
   - on known partial terminal I/O, mark the surface desynchronized and force a later full repaint

J. Atomically commit logical host state
   - visible occurrence tree and bindings
   - geometry/placement/presentation
   - Connector selection, projection, statuses, and subscriptions
   - viewport clamping/follow result
   - damage/screen baseline
   - visible frame revision
   - committed epoch through the captured boundary

K. Publish post-commit observations
   - Connector status events
   - counters/traces
   - release old snapshots/resources
   - leave newer epochs pending
```

Steps F through J replace every older algorithm that placed layout before candidate projection/activation.

### 6.3 Source snapshot consistency

A Source snapshot is immutable and revisioned. If a Source mutates after the attempt captures revision R:

- the attempt may validly commit a projection of R;
- the mutation advances the affected host's pending epoch;
- the next frame projects the later revision;
- no attempt reads half of two Source revisions.

Snapshot acquisition copies/retains chunk metadata, not the full Source text.

### 6.4 Candidate Connector activation is inside convergence

A requested Connector cannot become committed active until its projection has successfully participated in measurement, placement, and paint.

For a switch from Connector A to B:

1. A remains the committed visible Connector.
2. B is the requested candidate and receives a Source snapshot.
3. B is projected under the candidate width/viewport/theme.
4. If B succeeds, candidate layout/paint uses B and commit atomically swaps activation/subscriptions.
5. If B has an operational failure, the candidate records B as failed and reuses A's projection under the candidate geometry where valid.
6. If there is no old Connector, the ContentPort uses its defined empty/fallback projection.
7. Unrelated structural/state work may still commit.

Activation is therefore transactional without making an ordinary parser/Connector error a whole-host frame failure.

### 6.5 Projection key

A Connector projection cache key must include every semantic input that can change its output, at least:

```text
Source identity + Source content generation + Source revision
Funnel identity/configuration
candidate offered width or constraint key
host theme/style-resolution generation
relevant read-only viewport/materialization key
annotation/style semantic generation where separate
```

The implementation may split measurement and visible-window caches so scroll-only movement does not reparse or rewrap all content.

### 6.6 Measurement versus viewport materialization

Intrinsic/content extent measurement must not depend on the current scroll offset. The recommended split is:

```text
width-dependent projection/model
        -> intrinsic row/line metrics
viewport context
        -> visible-window materialization and paint selection
```

A virtualized Funnel may avoid materializing off-screen rows, but it must still provide deterministic extent/anchor information required by the viewport controller.

### 6.7 Convergence termination

The work loop terminates when no projection key, measurement, or placement relevant to the candidate changed.

The implementation must:

- process dirty work deterministically;
- avoid unconditional whole-tree passes once dependency metadata exists;
- count iterations and record the invalidation cause;
- impose a defensive iteration ceiling;
- treat exceeding the ceiling as an internal invariant/cycle diagnostic, not an ordinary user error or silent acceptance of unstable geometry.

Funnel projection for fixed inputs must be deterministic and side-effect free.

### 6.8 Logical versus physical atomicity

The logical frame commit is atomic. A terminal/OS write cannot in general be physically rolled back after a partial write. If that rare condition occurs:

- do not claim the old physical screen is intact;
- preserve or poison logical state according to the backend protocol;
- mark the physical surface unknown;
- force a full repaint on recovery;
- include the condition in the structured backend error.

This exception does not weaken candidate isolation for in-memory host state.

### 6.9 Capturing concurrent/new work

The frame commits only through its captured host epoch. Work accepted during preparation remains pending. Commit must not overwrite a newer pending epoch or clear the environment pending-host entry when newer work exists.

---

## 7. Retained ViewState

### 7.1 Public semantic model

`ViewState` is a host-owned retained override record created independently of a View:

```ts
const state = tui.viewState();
const view = View.container(child).state(state);

state.setGeometry({ /* typed patch */ });
state.setPresentation({ /* typed patch */ });
```

The `.state(state)` call creates a new semantic View value carrying the state `HandleId`. It does not embed mutable state data in the semantic node.

### 7.2 Why ViewState is not generic over node kind

A ViewState may be configured while unmounted and later remounted to another compatible physical node kind. Parameterizing it as `ViewState<ContainerKind>` would either prevent useful remounts or falsely imply that TypeScript can prove the eventual occurrence kind across the semantic DAG.

The final split is:

- TypeScript types the patch shape, property names, and value domains;
- Rust owns an exhaustive `NodeKind -> StateCapabilitySet` table;
- H3 prepare validates all retained overrides when attaching/remounting;
- mounted mutations validate against the desired binding synchronously.

“Typed” never means “stringly typed `setProperty(name, any)`.”

### 7.3 Lifecycle

A ViewState can be:

```text
unbound
prepared for desired binding
bound in desired structure only
visible-bound (possibly also desired-bound elsewhere during remount)
disposing
disposed
```

Rules:

- an unbound ViewState may accumulate any schema-valid override;
- H3 prepare accepts a binding only if every live override is supported by the target node capability set;
- after H3 commit, mutations validate against the desired target kind, even while the old visible occurrence remains on screen;
- a same-host remount is transactional and preserves overrides;
- duplicate use in one desired candidate is rejected;
- disposal is rejected while either desired-bound or visible-bound.

### 7.4 Patch atomicity

Each state method is all-or-nothing:

1. validate the complete patch shape and values;
2. validate handle/lifecycle and desired target capabilities;
3. enqueue/apply one native mutation record;
4. advance the host pending epoch only when effective state can change;
5. return one wake disposition.

A partially valid patch never partially mutates the ViewState.

### 7.5 `undefined`, `null`, and clear

The rules are exact:

- property omitted or `undefined` in a patch: do not change that override;
- `null`: an explicit semantic “none” value only for a field whose generated schema declares it nullable;
- `clearGeometry(...)` / `clearPresentation(...)`: remove the selected retained overrides;
- no-key clear form, where exposed, removes all overrides in that domain;
- clearing reveals the immutable base value supplied by the currently desired View;
- `null` is never a generic spelling of clear.

A generated field table must state nullability. APIs reject `null` for all other fields.

### 7.6 Base plus override

Every physical state-capable occurrence has:

```text
immutable View-derived base box/presentation
                +
retained ViewState overrides
                =
candidate effective occurrence state
```

A remount may change the base while preserving the retained override. Clearing after remount therefore reveals the new View's base, not the base from the previous mount.

### 7.7 Capability classes

The H3 native node-kind enum must be covered by an exhaustive Rust match. No wildcard arm is allowed in the capability definition.

The semantic classes are:

| Node class | ViewState support |
|---|---|
| structural-only node, fragment, component indirection, identity/boundary with no physical layout occurrence | none; `.state()` attachment is an H3 prepare error |
| physical text/content leaf | base box geometry, box presentation, and leaf presentation fields defined by the schema |
| physical container/layout node | base box geometry and box presentation; container-specific mutable fields only when explicitly in PERF-13's generated schema |
| ContentHost occurrence | base box geometry and box presentation; content text semantics remain Funnel/annotation responsibility |
| ScrollPane/RowViewport physical occurrence | base box geometry and presentation; scroll offset/follow state is not ViewState |
| spacer/empty physical layout item | only capabilities that affect its real box; no text/content fields |

The implementation documentation generated from the exhaustive table must list every concrete node kind at the API-H3 baseline. Adding a node kind fails compilation/tests until its capability class is chosen.

### 7.8 Rust-side effect classification

TypeScript does not send “paint-only,” “layout,” or similar authority flags. Rust metadata classifies each effective property transition into one or more effects:

```text
projection invalidation
measurement invalidation
placement invalidation
paint/damage invalidation
hit-test/focus-map invalidation, where in scope
```

Classification may depend on old and new values. Examples:

- border edge presence/width: geometry, measure/place/paint;
- border glyph/style/color with unchanged edge presence: paint;
- padding: geometry, measure/place/paint;
- foreground/background: paint;
- bounds constraints: measure/place/paint as derived;
- setting the same effective value: no frame work.

### 7.9 Dirty propagation

The native layout engine owns dependency metadata and propagation. At minimum it must correctly handle:

- changed child intrinsic size invalidating measuring ancestors;
- changed parent constraints invalidating affected descendants/projections;
- moved/resized nodes damaging both old and new regions;
- paint-only changes avoiding structure rebuild and unnecessary measurement;
- selector/theme dependencies conservatively repainting a subtree until a finer index is justified.

Exact indexing is implementation-local; correctness and counters are not.

---
## 8. OccurrenceBox and `Decorated` migration

### 8.1 Final ownership model

Model A is normative: mutable state attaches to the **underlying physical occurrence**, and that occurrence always owns an initially empty/base box state.

```text
physical occurrence
  ├─ semantic base geometry
  ├─ semantic base presentation
  ├─ retained ViewState overrides, if attached
  ├─ measured/placed geometry
  └─ paint state
```

Calling `state.setGeometry({ borderEdges: ... })` may introduce or remove border cells without creating, deleting, or reidentifying an occurrence.

`.state()` does not create a stateful-box wrapper. State is not limited to overriding a pre-existing `Decorated` wrapper.

### 8.2 What `OccurrenceBox` means

Every state-capable physical node has an `OccurrenceBox` even when all values are visually empty/default. It is a native layout/render record, not necessarily a public View kind.

It owns box-model properties whose semantics surround that occurrence's content, including the PERF-13 geometry/presentation fields already specified in the baseline. The exact struct layout remains implementation-local.

### 8.3 Compatibility normalization for old decorators

Do not begin PERF-13 by deleting `Decorated`. Instead classify every current decorator/wrapper:

#### Property-only decorators

A wrapper is property-only when its observable behavior is fully expressible as base `OccurrenceBox` fields on the decorated owner, for example a pure border, padding, background, or bounds wrapper with no independent identity/interaction semantics.

Structural prepare/lowering normalizes it into the owner's immutable base state. It does not survive as an extra physical occurrence in the new path.

#### Semantically distinct wrappers

A wrapper remains a real occurrence until a dedicated migration proves equivalence if it owns behavior such as:

- clipping/viewport identity;
- focus or hit-test boundary;
- independent child allocation semantics;
- scrolling state;
- event routing;
- a transform or composition rule not representable by the box schema.

The presence of such wrappers does not prevent the underlying physical node from owning its own OccurrenceBox.

### 8.4 Normalization order

When legacy builder/decorator APIs compose static values, their established precedence must be preserved. Structural prepare produces one canonical base record in the same observable order, then retained overrides are applied on top.

```text
legacy static builder/decorator chain
        -> canonical View-derived base
        -> retained override
        -> effective candidate state
```

The implementation must not accidentally make decorator call order irrelevant when it was previously observable.

### 8.5 Migration safety rules

1. No old and new record may both add the same padding/border extent.
2. Damage is computed from old and new effective boxes, not wrapper-count changes.
3. Semantic View equality/hash/identity behavior is preserved until the corresponding public builder migration is intentionally landed.
4. A compatibility adapter may exist for one or more tranches, but there is one native owner for each effective property.
5. Golden layout and paint parity tests are required before removing each old wrapper representation.
6. A debug counter records any frame that still lowers through a legacy Decorated compatibility case.

### 8.6 Dynamic border example

```ts
const s = tui.viewState();
const v = View.container(child).state(s);

s.setGeometry({
  borderEdges: { top: true, right: true, bottom: true, left: true },
});
```

This changes the same occurrence's box geometry, invalidates measurement/placement as classified by Rust, and repaints. It does not publish a new View and does not require a pre-existing border wrapper.

---

## 9. Content entity model and public API

### 9.1 Canonical nouns

The content model has four distinct nouns:

```text
Source       retained mutable bytes/text/semantic annotations
Funnel       immutable Source-neutral projection configuration
ContentPort  host-owned structural mount point
Connector    host-owned link: Source + Funnel + ContentPort
```

The canonical public shape is:

```ts
const source = TextStreamSource.create({
  retention: { /* policy */ },
});

const funnel = TextFunnel.plain({
  wrap: "word",
});

const port = tui.contentPort({
  /* port options */
});

const connector = port.connect(source, funnel);
const view = View.content(port);

connector.activate();
source.append("hello");
```

`port.connect(source, funnel)` is the normative API. `source.funnel(...)` is not used because it makes a Source-bound object look like the immutable Funnel value from the entity model.

If a fluent Source-bound convenience is later justified, its type must be named `ContentFeed`, `SourceBinding`, or equivalent, and `port.connect` must still produce the Connector explicitly.

### 9.2 Funnel properties

A Funnel is:

- immutable;
- Source-neutral;
- host-neutral at the semantic/configuration level;
- reusable by multiple Connectors;
- deterministic and side-effect free for fixed inputs;
- represented by a generated/closed native-known configuration union in PERF-13 v1.

A compiled native Funnel plan may be interned per environment. That is an implementation detail and does not change value semantics.

Arbitrary JavaScript callbacks in the projection hot path are out of scope for PERF-13.

### 9.3 Source creation without Tui ownership

`TextStreamSource.create(...)` obtains the current package/runtime environment, not a Tui host. All Tuis created in that same JavaScript environment can connect to it.

Implementations with explicit dependency injection may expose an environment factory internally for tests. The public source must never be silently rebound to whichever Tui first connects.

### 9.4 Structural attachment

Only the ContentPort is attached to the semantic View:

```ts
View.content(port)
```

The semantic node carries the ContentPort framework `HandleId`. Source, Funnel, Connector, projection cache, viewport offset, and active selection do not enter the semantic DAG.

### 9.5 One Source, multiple hosts

A Source may feed Connectors belonging to several Tui hosts in the same environment. Each host may use a different Funnel, width, theme, viewport, and active revision timing. Therefore:

- Source storage/snapshots are host-independent;
- projection and host-native style resolution are Connector/host work;
- one Source mutation can mark several hosts pending;
- Source annotations cannot contain a Tui-host-native Style ID;
- disposing one host leaves the Source and other hosts intact.

### 9.6 Coldness and membership

`port.connect(source, funnel)` creates persistent Connector membership, but membership alone is cold:

- no projection is created;
- no Source snapshot is retained for display;
- no host subscription is installed;
- Source mutation does not wake this Connector's host.

Activation request and a desired/visible port mount are both required before it becomes activation-pending.

---

## 10. ContentPort lifecycle

### 10.1 State model

A ContentPort has two related state dimensions:

#### Structural binding

```text
unmounted
prepared desired mount
requested/desired-mounted
visible-mounted
remount transition (old visible + new desired)
unmount transition (old visible only)
disposed
```

#### Connector selection

```text
no requested Connector
requested Connector while unmounted
requested activation pending
committed visible Connector
requested switch while old Connector remains committed
requested Connector failed while old Connector remains committed
```

The native record stores these dimensions explicitly. They must not be collapsed into one ambiguous `active` boolean.

### 10.2 Creation and connection

A new ContentPort is alive and unmounted. It may acquire Connector membership and a requested selection before any View containing it is published.

Connecting while unmounted does not subscribe or project.

### 10.3 Activation while unmounted

`connector.activate()` on an unmounted port:

- synchronously validates liveness, ownership, membership, Source, and Funnel configuration;
- records that Connector as the port's latest requested selection;
- gives the Connector public phase `waiting-for-mount`;
- keeps it cold;
- does not create a projection;
- does not subscribe the host to the Source;
- does not require a frame solely to display nonexistent content.

A later desired mount advances host work. Before the first committed mounted frame, the requested Connector is prepared inside convergence.

### 10.4 Initial mount

When H3 commits a desired tree containing the port:

- desired binding is established;
- the host becomes pending;
- a requested Connector, if any, becomes activation-pending for that host;
- the candidate frame snapshots/projects it;
- only successful frame commit establishes visible binding, projection, active subscription, and `active` status.

With no requested Connector, the committed port displays its defined empty/fallback content.

### 10.5 Unmount

When a desired publication removes the port:

- requested selection and Connector membership remain retained;
- the old visible port and Connector remain visible until the unmounting frame commits;
- Source changes may continue to wake the host while that old Connector remains visible;
- successful frame commit drops projection and committed Source subscription;
- the requested Connector transitions to `waiting-for-mount` and becomes cold;
- viewport state remains owned by its viewport controller according to that controller's lifecycle.

### 10.6 Remount

A same-host remount moves one port from its old occurrence to one new compatible occurrence transactionally:

- H3 prepare proves single use and compatibility;
- desired binding points to the new occurrence;
- old visible binding remains until commit;
- the selected Connector is projected under the new candidate width/viewport;
- commit swaps binding/projection with no duplicate active mount.

### 10.7 Deactivation while unmounted

Deactivation clears the requested selection. No frame is necessary unless an old visible binding/Connector is still in an unmount transition. All membership remains until explicit Connector disposal.

### 10.8 Disposal rules

`ContentPort.dispose()` succeeds only when:

- no desired binding exists;
- no visible binding exists;
- no structural transition lease exists;
- no Connector membership exists;
- the host is live and unpoisoned enough to perform disposal.

Otherwise it fails synchronously with a specific mounted/in-use code. There is no hidden Connector cascade.

### 10.9 Host disposal

Host disposal is the explicit owner-death exception:

1. prevent new publication/control work;
2. abort prepared host publications/frames;
3. remove visible and candidate Source subscriptions;
4. release Connector projections/snapshots;
5. invalidate Connector, Port, and ViewState generations;
6. remove the host from the environment pending set;
7. leave environment Sources alive.

---

## 11. Connector lifecycle and transactional switching

### 11.1 Connector record

A Connector binds exactly:

```text
one ContentPort
one Source
one immutable Funnel specification
```

It is host-owned because activation, projection, width, viewport context, and visible status are host-specific.

### 11.2 Public status model

A Connector status must expose enough truth to distinguish request from visibility. The exact API spelling may follow project conventions, but the semantic fields are:

```ts
type ConnectorStatus = {
  readonly phase:
    | "idle"
    | "waiting-for-mount"
    | "activation-pending"
    | "active"
    | "failed"
    | "disposing"
    | "disposed";
  readonly requested: boolean;
  readonly visible: boolean;
  readonly projectedSourceRevision?: bigint;
  readonly error?: ConnectorOperationalError;
};
```

During a failed switch, old Connector A may report `active/visible`, while requested Connector B reports `failed/requested/not visible`.

### 11.3 Latest request wins

Multiple activate/deactivate requests accepted before the same frame boundary are coalesced per port. The last request within the captured host epoch wins. Intermediate Connectors are not projected merely because they were transiently requested.

### 11.4 Switch success

A successful switch commits as one logical operation:

- B's candidate projection becomes visible;
- B becomes the committed Connector;
- B's Source subscription becomes active;
- A's subscription/projection is released or retained only by immutable history snapshots;
- A becomes idle;
- status events are emitted after commit.

### 11.5 Switch failure

An operational failure preparing B:

- records a structured error on B;
- keeps B as the requested Connector unless user code deactivates/selects another;
- leaves A committed, visible, and subscribed;
- allows unrelated frame changes to commit using A as fallback;
- avoids immediate busy retries.

B may be retried when:

- its Source revision advances;
- width/theme/viewport inputs relevant to the failure change;
- the port remounts;
- user code invokes an explicit retry;
- a retry policy for that Connector error class says it is ready.

### 11.6 Deactivation

Deactivation requests no committed Connector. The old Connector remains visible until a frame successfully paints the port's empty/fallback state and commits the subscription change.

### 11.7 Connector disposal

Disposing an idle Connector with no candidate/visible lease may complete synchronously.

Disposing a requested or committed Connector is transactional:

- mark it disposing so new operations reject;
- request deactivation/removal from its port;
- preserve old visible content until a frame can commit the removal;
- release membership and invalidate its generation after commit.

`connector.dispose()` synchronously enters `disposing` and rejects further Connector operations. Native identity is finalized after the removal frame commits. Callers that require completion call `tui.flush()` and then observe `disposed`; the wrapper must not claim final disposal while visible native state still relies on it.

### 11.8 Source disposal interaction

Connector membership counts as Source use even when cold. Consequently `Source.dispose()` rejects while any Connector still points to it. This prevents a cold Connector from becoming a delayed dangling activation.

---

## 12. Environment-owned Source registry and storage

### 12.1 Ownership hierarchy

```text
NativeEnvironment
  ├─ SourceRegistry
  │    ├─ Source S1
  │    └─ Source S2
  ├─ TuiHost A
  │    ├─ ViewStates
  │    ├─ ContentPorts
  │    └─ Connectors
  └─ TuiHost B
       ├─ ViewStates
       ├─ ContentPorts
       └─ Connectors
```

A Connector retains a strong native reference/use count to its Source registry entry. A Source subscriber record refers weakly to host/Connector generations.

### 12.2 Source mutation linearization

Each Source has a mutex or equivalent serial mutation guard and a monotonically increasing 64-bit Source revision.

For every Source operation:

1. validate environment and Source generations;
2. acquire the Source mutation guard;
3. validate operation ranges/limits;
4. apply text/annotation/retention mutation atomically;
5. assign the next native Source revision;
6. capture eligible weak subscribers;
7. release the Source guard;
8. mark live affected hosts pending;
9. return revision plus one environment wake disposition.

This lock acquisition/commit order is the operation's linearization order. ABI v1 has no JavaScript-supplied sequence lanes.

Concurrent future workers have no pre-existing total “JavaScript call order”; the native Source serialization defines one deterministic total order.

### 12.3 Lock-order rule

Source mutation must not hold a Source mutex while taking a host frame lock. It copies weak subscriber tokens, releases Source storage, then marks hosts through atomics/environment scheduler state.

Frame snapshot acquisition likewise takes an immutable snapshot/Arc and releases the Source guard before layout or projection.

This avoids Source↔host lock inversion when one Source feeds several hosts.

### 12.4 Snapshot shape

A snapshot contains or identifies:

- Source identity;
- content generation;
- Source revision;
- absolute retained head/tail coordinates;
- immutable references to text chunks/indexes;
- immutable semantic annotation indexes;
- retention metadata needed for coordinate interpretation.

A snapshot must not copy the full text on each frame. Old snapshots remain valid through ref-counted immutable storage while a candidate frame/history entry uses them.

### 12.5 Text storage constraints

The concrete chunk/deque/rope design is implementation-local, but it must provide:

- append proportional to appended payload plus bounded indexing work;
- head retention/truncation without moving all remaining bytes;
- snapshot creation without full-content copy;
- replace without mutating snapshots already in use;
- byte/range validation at the native boundary;
- bounded fragmentation and measurable compaction policy;
- counters for retained bytes, chunk count, copied bytes, and dropped head bytes.

### 12.6 Coordinates

Text Source storage uses monotonically increasing absolute UTF-8 byte coordinates within one append content generation:

```text
head_offset <= annotation/content offset <= tail_offset
```

This preserves external rolling-stream coordinate use cases and removes the need for a separate public `StreamSnapshot` mutation architecture.

`replace()` starts a new content generation. Any externally retained coordinate must include/validate that generation. Operation-local annotation ranges in append/replace payloads are converted to absolute Source coordinates during the atomic mutation.

Retention never splits a UTF-8 scalar. Grapheme-aware policies may choose a later safe boundary where required by presentation semantics.

### 12.7 No-op semantics

An operation that is provably semantically empty may return the current Source revision and no wake. Otherwise every successful mutation receives a new revision, even when a costly deep equality check might discover equal content. Correct ordering is preferred over speculative deduplication.

### 12.8 Source disposal

Disposal validates:

- no live Connector membership;
- no prepared connection transaction;
- correct environment/generation;
- live environment.

Immutable snapshots already detached into committed history may retain their chunk Arcs after the Source registry entry is disposed. They do not keep the mutable Source identity usable.

---
## 13. Semantic annotations and the content sidecar ABI

### 13.1 Annotation ownership

Annotations are part of the Source snapshot. They describe semantic ranges in Source content and survive independently of any Tui host.

They must never store:

- a TuiHost ID;
- a host-native `StyleId`;
- a resolved palette slot tied to one theme;
- an occurrence key;
- a viewport-relative row coordinate.

They may store an environment-level immutable semantic-style handle if the handle resolves to a host-independent descriptor and remains valid for the Source lifetime. Direct descriptors are also valid.

### 13.2 Exact semantic categories

The current Iyon annotation declarations captured in the repository-audit appendix are mandatory inputs to migration. Every current declaration must lower losslessly to one of these wire-semantic categories:

| Category | Meaning | Head-truncation policy |
|---|---|---|
| continuous style span | visual text style over a half-open text range | clip the start to the new head; remove when empty |
| continuous semantic/link span | metadata remains meaningful for the surviving text | clip the start to the new head; remove when empty |
| atomic range | meaning requires the complete annotated token/object | drop if any part is truncated |
| point/anchor | zero-width marker at an absolute coordinate | drop when its coordinate is before the new head; retain otherwise |

The baseline Iyon visual annotations are continuous semantic/style ranges and therefore clip at the retained head. An implementation must not make all future kinds clip by default: the generated kind table carries one of the explicit policies above and has no wildcard fallback.

### 13.3 Range representation

Native annotation ranges are half-open absolute UTF-8 byte ranges within a Source content generation:

```text
[start_byte, end_byte)
```

For point annotations, `start_byte == end_byte`.

Public JavaScript adapters may accept the existing Iyon coordinate convention. They must convert once at the operation boundary and validate:

- ordered range;
- range within the supplied append/replace payload or referenced Source generation;
- UTF-8 scalar boundaries after encoding;
- kind-specific zero-length legality;
- payload index/length bounds.

The native Source stores absolute coordinates. FFI append/replace sidecars use operation-local byte offsets so the common case fits `u32`; native adds the append base after validation. Source absolute offsets and revisions remain `u64`.

### 13.4 Head truncation

For a new retained head H:

```text
end <= H
    remove the annotation

start < H < end, continuous kind
    set start = H

start < H < end, atomic kind
    remove the annotation

point < H
    remove the annotation

start >= H
    retain unchanged
```

Clipping and text truncation happen in the same Source mutation transaction and produce one Source revision. No snapshot can observe truncated text with stale out-of-range annotations.

### 13.5 Semantic style representation

The content ABI represents a style semantically, not as a host-renderer ID. A semantic style descriptor contains the fields used by the current Iyon annotation model, normalized into:

```text
optional foreground semantic color
optional background semantic color
text-attribute bitset
optional underline semantic color/style where supported
optional semantic role/token for theme resolution
```

A semantic color is a tagged value, for example default/inherit, indexed terminal color, RGB, or named theme role. Named roles are environment-level immutable strings/handles, not host palette indices.

During Connector projection for host H:

```text
Source semantic annotation
        + H theme/style generation
        -> H-native resolved paint style
```

Changing a host theme invalidates the Connector projection/style-resolution key without mutating the Source.

### 13.6 Fixed annotation record layout

ABI v1 uses a fixed-width record array plus a byte payload table. It does not pass JavaScript objects or JSON.

```c
/* Native-endian, same-process ABI. Eight uint32 lanes per record. */
typedef struct IyonTuiAnnotationRecordV1 {
    uint32_t kind;
    uint32_t flags;
    uint32_t start_byte;
    uint32_t end_byte;
    uint32_t payload_offset;
    uint32_t payload_length;
    uint32_t aux0;
    uint32_t aux1;
} IyonTuiAnnotationRecordV1;
```

Semantics:

- `kind` selects a generated annotation schema entry;
- `flags` contains only ABI-defined flags and kind-independent truncation/range bits;
- `start_byte/end_byte` are operation-local for append/replace calls;
- `payload_offset/payload_length` index the separate annotation payload byte array;
- `aux0/aux1` carry fixed common values such as a semantic-style table index; otherwise zero;
- unknown kind, flag, reserved lane, or malformed payload is rejected before Source mutation.

The TypeScript wrapper uses one `Uint32Array` with `count * 8` lanes or an exactly equivalent set of parallel `Uint32Array` lanes generated from the same ABI schema. The native ABI is defined by the C record semantics and metadata-reported size, not by a handwritten JS object layout.

### 13.7 Kind payloads

Kind-specific payloads are compact generated binary records/string-table slices. ABI v1 forbids per-annotation JSON parsing.

The migration generator must emit, from one schema:

- TypeScript annotation discriminated unions/adapters;
- TypeScript sidecar encoder;
- C/Rust kind constants and record validation;
- Rust semantic annotation enums;
- truncation-policy table;
- round-trip fixtures for every current Iyon kind and field.

The repository-audit appendix is the completeness checklist. No current annotation kind or field may be silently discarded.

### 13.8 Projection, clipping, and viewport

Source-range clipping is retention semantics. Viewport clipping is paint/materialization semantics and must not rewrite Source annotations. A Connector maps absolute Source ranges into projected rows/cells for the current width, then intersects them with the read-only viewport window for paint.

### 13.9 Annotation limits

Native validation applies explicit configurable limits before allocation:

- maximum annotation count per call;
- maximum sidecar bytes;
- maximum individual payload bytes;
- maximum total Source annotation bytes if retention requires one.

Limit failure is a recoverable `LIMIT_EXCEEDED`/`PAYLOAD_TOO_LARGE` class, not `OUT_OF_MEMORY`.

---

## 14. ContentPort, viewport, ScrollPane, and History

### 14.1 Single owner for viewport control

ContentPort owns only:

- structural mount binding;
- candidate/visible allocation supplied by layout;
- clip binding needed to project/paint within its occurrence;
- Connector selection/projection association.

It does **not** own:

- scroll offset;
- follow-end state;
- user scroll intent;
- scroll anchors;
- History navigation position.

Those belong to the existing ScrollPane/RowViewport controller, or to one retained viewport-control record introduced there.

### 14.2 Read-only Connector context

Projection receives a read-only context derived from candidate layout and the viewport controller:

```ts
type ContentProjectionContext = {
  readonly offeredWidth: number;
  readonly allocatedHeight: number;
  readonly clipRect: Rect;
  readonly viewportStart: bigint | number;
  readonly viewportLength: number;
  readonly followEnd: boolean;
  readonly themeGeneration: bigint;
};
```

Exact scalar widths may differ internally. The ownership rule does not.

A Connector cannot mutate scroll state while projecting. It returns extent/anchor information and visible materialization; the viewport controller applies clamping/follow logic in the candidate transaction.

### 14.3 Connector switch preserves viewport state

Because viewport control is not Connector-owned, switching Connectors preserves offset/follow intent. At candidate commit:

- the controller clamps an out-of-range offset to the new extent;
- follow-end moves to the new end according to existing semantics;
- a source/funnel-specific anchor may be resolved if the viewport already supports anchors;
- failure to activate the new Connector leaves the old projection and viewport basis intact.

### 14.4 Scroll-only invalidation

A scroll-offset change should normally invalidate visible-window materialization and paint, not reparse or rewrap all content. Width change invalidates the width-dependent projection key. Follow-state changes are viewport control mutations, not Source mutations.

### 14.5 History integration

History consumes the same committed content/viewport model. It must not create a second Source subscriber or content scheduler.

The integration contract is:

1. live content is projected through the active Connector;
2. when an item/unit becomes historical, History captures an immutable committed projection/snapshot descriptor at a specific Source revision;
3. History may retain immutable Source chunks/semantic annotations or a frozen projected representation according to its memory policy;
4. navigation uses the same viewport/controller concepts;
5. History never receives high-volume text through the superseded host payload bridge;
6. freezing a live unit and releasing its Connector subscription is transactional with the relevant host update.

The current Iyon History call sites listed in the audit appendix must be migrated before the legacy path is deleted.

### 14.6 Markdown

Markdown is a Funnel, not a separate stream architecture. PERF-13 includes a `MarkdownFunnel` migration because Iyon consumes Markdown-formatted live/history content at the audited baseline.

It must:

- consume immutable Source snapshots and semantic annotations;
- produce width-dependent projected rows/metrics;
- preserve current visual annotation behavior;
- be deterministic for a fixed projection key;
- cache parser/layout state without making the Source host-specific;
- participate in the same convergence and viewport materialization path as plain text.

An existing Markdown parser/algorithm may be reused internally. Its old host scheduling or high-volume payload route may not survive.

---

## 15. Direct FFI ABI v1

### 15.1 Scope of FFI

Mandatory direct FFI is used for high-volume Source payload operations. Node-API/generated control bindings may remain for object creation, structural publication, ViewState control, Connector control, queries, and error records where payload overhead is not the bottleneck.

There is one native runtime and one set of registries behind both surfaces.

### 15.2 Same native artifact

`bun:ffi` loads the exact same staged `.node` artifact that exports the Node-API control module. The artifact exports:

- its normal Node-API initialization symbol;
- default-visible, unmangled C ABI symbols for PERF-13;
- one ABI metadata/probe symbol.

A second dylib/so is prohibited for PERF-13 unless the same-artifact platform proof fails and an architecture review explicitly accepts the packaging/lifetime cost. It must not be introduced as an implementation convenience.

### 15.3 One artifact locator

One generated locator module is shared by the Node-API loader and `ffi.ts`:

```ts
resolveNativeArtifact(import.meta.url): {
  absolutePath: string;
  packageBuildId: string;
  platform: string;
  arch: string;
}
```

It must cover:

- repository development build;
- `native:stage` output;
- test fixtures;
- published package layout;
- every currently supported macOS and Linux architecture.

No second hand-maintained platform/filename switch is allowed in `ffi.ts`. The locator canonicalizes to one real absolute path so the OS loader cannot map the same binary twice through a staging symlink and a package path.

The FFI library handle remains open for the environment lifetime. It is not repeatedly opened/closed per Source or call. Mapping the artifact must not itself create a second semantic runtime: environment creation remains a Node-API/control operation, metadata probing is side-effect free, and every payload symbol requires a valid environment handle.

### 15.4 Metadata handshake

Before the first payload call, `ffi.ts` invokes a metadata function and validates at least:

```text
magic
ABI major/minor
pointer width
native endianness marker
sizes/alignments of every public ABI record
annotation record lanes/version
status table version
native package build ID/schema fingerprint
```

A mismatch prevents Source payload use with `ABI_MISMATCH`; it never attempts a best-effort call.

Suggested symbol:

```c
uint32_t iyon_tui_perf13_abi_metadata_v1(
    IyonTuiPerf13AbiMetadataV1* out_metadata,
    uint32_t out_size
);
```

Exact generated symbol spelling is not architectural, but once ABI v1 ships, symbol and layout are compatibility commitments.

### 15.5 Source identity in every payload call

Every Source payload call receives:

```text
environment_slot: u32
environment_generation: u32
source_slot: u32
source_generation: u32
```

There is no TuiHost identity in a Source mutation call. Native subscriber state determines affected hosts.

### 15.6 Mutation result

Payload calls return a status code and write a fixed result record supplied by the caller:

```c
typedef struct IyonTuiSourceMutationResultV1 {
    uint32_t source_revision_lo;
    uint32_t source_revision_hi;
    uint32_t environment_wake_epoch_lo;
    uint32_t environment_wake_epoch_hi;
    uint32_t flags;      /* includes SCHEDULE_ENVIRONMENT_DRAIN */
    uint32_t reserved0;
} IyonTuiSourceMutationResultV1;
```

The wake epoch is diagnostic/edge coordination information, not a caller ordering token. JavaScript queues the environment broker when the schedule flag is set.

### 15.7 Append/replace signatures

Conceptually:

```c
uint32_t iyon_tui_source_append_utf8_v1(
    uint32_t environment_slot,
    uint32_t environment_generation,
    uint32_t source_slot,
    uint32_t source_generation,
    const uint8_t* text_ptr,
    uint32_t text_len,
    const IyonTuiAnnotationRecordV1* annotations_ptr,
    uint32_t annotation_count,
    const uint8_t* annotation_payload_ptr,
    uint32_t annotation_payload_len,
    IyonTuiSourceMutationResultV1* out_result
);
```

`replace`, explicit head truncation, and other baseline-required payload mutations follow the same identity/result conventions. Payload lengths are bounded `u32`; Source absolute coordinates and revisions are `u64` split only where the JS/FFI scalar contract requires it.

### 15.8 Pointer and reentrancy contract

Input pointers are borrowed only for the duration of the synchronous FFI call. Native must finish copying/adopting validated bytes into owned Source chunks before return.

The FFI call:

- does not call JavaScript;
- does not retain TypedArray pointers;
- does not run layout/projection;
- does not flush hosts;
- does not invoke user callbacks;
- may mark native hosts pending and return the one wake flag.

The wrapper keeps all TypedArrays strongly reachable until return.

### 15.9 Encoding

TypeScript uses `TextEncoder` or an equivalent correct UTF-8 encoder. Scratch-buffer sizing/pooling is benchmark-driven. Native validates UTF-8 when the symbol contract says UTF-8; invalid input produces `INVALID_UTF8` without mutation.

### 15.10 Linearization: no caller sequence

ABI v1 has no `sequence_low/high` input.

The native Source mutation critical section defines total order, and the assigned Source revision proves that order. This is correct for one JS thread and for future concurrent workers. Caller-supplied sequence values would either duplicate that authority or require a cross-worker ordering definition that does not exist.

### 15.11 Status classes

The recoverable status table includes precise families such as:

```text
OK
INVALID_ARGUMENT
ABI_MISMATCH
WRONG_ENVIRONMENT
STALE_ENVIRONMENT
STALE_SOURCE
SOURCE_DISPOSED
SOURCE_IN_USE
INVALID_UTF8
INVALID_RANGE
UNKNOWN_ANNOTATION_KIND
INVALID_ANNOTATION_PAYLOAD
LIMIT_EXCEEDED / PAYLOAD_TOO_LARGE
RUNTIME_POISONED
INTERNAL_PANIC (only when safely contained by the build)
```

The generated TypeScript mapping must be exhaustive. Unknown status values are ABI mismatch/fatal diagnostics, not generic success/failure.

### 15.12 OOM policy

`OUT_OF_MEMORY` is removed from the advertised recoverable ABI.

Ordinary Rust allocation may invoke the allocation-error handler rather than return `Result`; claiming recoverable OOM would be false unless every relevant allocation path deliberately used fallible reservation and propagated failure.

PERF-13 instead:

- validates explicit payload/retention limits before large allocation;
- may use `try_reserve` where a normal capacity/limit error is useful;
- treats genuine allocator exhaustion as runtime/process-fatal;
- never returns a fabricated OOM status after partial mutation.

### 15.13 Panic boundary

No panic may unwind across C ABI or Node-API. Depending on build panic strategy, entrypoints either catch and poison safely or abort according to the configured fatal policy. The ABI metadata reports the relevant runtime build mode for diagnostics, not for application recovery logic.

### 15.14 Platform feasibility gate

Before the content payload tranche merges, CI must build the real staged artifact and prove on every supported target:

1. Node-API loads the artifact through the ordinary package path;
2. `bun:ffi` resolves the same absolute path;
3. the metadata symbol is visible;
4. the metadata build/schema fingerprint matches the JS package;
5. one append call crosses the real ABI and wakes a test host;
6. packaged/published layout uses the same locator.

A small Linux same-suffix/shared-object proof is recorded in the research appendix, but it does not replace the real target matrix.

---
## 16. Legacy stream, Markdown, History, and consumer migration boundary

### 16.1 Final architectural boundary

PERF-13 completion allows old **algorithms** to survive behind the new abstractions, but not old **runtime architecture**.

May survive internally:

- a proven text chunk/index algorithm;
- an immutable `StreamSnapshot`-like value used inside Source snapshots or Funnel projection;
- a Markdown parser;
- line wrapping, annotation interval, or history compaction utilities;
- compatibility type aliases that carry no separate identity/lifecycle.

Must disappear as independent paths:

- a public high-volume mutation API that bypasses Source;
- an old native stream registry parallel to SourceRegistry;
- an old JS→native text payload bridge parallel to direct FFI;
- an old host stream scheduler/dirty flag;
- View rebuilding whose purpose is only to deliver changing text;
- a second annotation storage/coordinate authority;
- History or Markdown subscriptions that bypass Connector/ContentPort;
- a production double-dispatch mode that sends each mutation through both paths.

### 16.2 `TextStream.update()` compatibility

Where public compatibility requires it:

```text
TextStream.update(snapshot/text)
        -> validate/translate legacy arguments
        -> Source.replace(text, annotations)
        -> return new-path result
```

The adapter owns no storage, native identity, queue, or repaint behavior. It may be deprecated according to package policy, but its continued presence does not violate one architecture when it is a pure adapter.

### 16.3 Monotonic external coordinates

Rolling consumers that previously depended on monotonic `StreamSnapshot` coordinates migrate to Source content generation + absolute UTF-8 offsets. That capability is part of the new Source model; it is not grounds for retaining a second runtime path.

### 16.4 Iyon migration is part of PERF-13

The repository audit identifies current Iyon uses of:

- TUI stream/text update APIs;
- annotations and styles;
- Markdown rendering;
- History/freeze behavior;
- viewport/scroll integration;
- package dependency pinning.

PERF-13 is not complete until those call sites use Source/Funnel/Port/Connector or a pure adapter terminating there, and tests prove current visible behavior.

### 16.5 Temporary dual-path rule

A temporary old/new implementation may exist only under these conditions:

- it is confined to an explicitly named tranche;
- only one path is authoritative in production;
- shadow comparison is test/debug-only and does not mutate two runtimes;
- counters reveal every old-path invocation;
- the tranche has a deletion gate and owner;
- no new consumer is allowed to adopt the old path.

### 16.6 Completion deletion proof

The final cleanup MR must include:

- code search showing no production references to superseded payload entrypoints/registries;
- zero legacy-path counters across the Iyon integration suite;
- package export audit showing no accidental second public API;
- native symbol audit showing obsolete high-volume bridge symbols removed where compatibility does not require them;
- memory/lifetime tests proving no Source/Connector snapshots are retained by deleted adapters.

---

## 17. Cross-plane mutation ordering and barriers

### 17.1 Host ingress order

Each accepted host-affecting operation obtains or advances a native host epoch. Per-plane queues retain enough ordering to produce the final state through a captured epoch.

The frame phase order is intentional:

```text
latest desired structure
then retained state/control values through capture
then Source snapshots
then projection/layout/paint
```

Operations that commute may be coalesced. A barrier between calls creates the observable boundary.

### 17.2 State coalescing

Repeated assignments to one ViewState property before the captured frame may collapse to the final value, provided:

- validation/error behavior already occurred synchronously at each call;
- no public API observes intermediate frame states without a barrier;
- effect classification compares the visible/candidate effective values correctly;
- counters distinguish accepted mutations from effective changes where needed.

### 17.3 Connector control coalescing

For one port, the latest activate/deactivate request through the captured epoch wins. Disposing a Connector is not freely coalescible past operations that rely on its liveness; lifecycle validation and transactional teardown remain ordered.

### 17.4 Source order

Source revisions are independent of host epochs. A frame records which Source revision each projection represents. Source mutation wakes affected hosts after the Source commit. If a host captures an earlier revision, the later host pending epoch guarantees follow-up work.

### 17.5 Barrier coverage

An explicit host barrier covers:

- structural publications accepted before capture;
- state/control/viewport mutations accepted before capture;
- Source mutations that had marked that host pending before capture;
- backend invalidations before capture.

It does not wait for unrelated future Source mutations or other hosts.

---

## 18. Error and lifecycle code contract

### 18.1 Why codes are explicit

Errors cross JavaScript, Node-API, C FFI, registries, H3 prepare, and automatic drains. String matching is prohibited. Each surface maps from one generated code catalog to typed errors/status records.

### 18.2 Required families

#### Environment/host

```text
WRONG_ENVIRONMENT
STALE_ENVIRONMENT
HOST_DISPOSED
STALE_HOST
RUNTIME_POISONED
```

#### Generic resources

```text
WRONG_RESOURCE_KIND
STALE_HANDLE
RESOURCE_DISPOSING
RESOURCE_DISPOSED
WRONG_HOST
```

#### Structural attachments

```text
DUPLICATE_VIEW_STATE_ATTACHMENT
DUPLICATE_CONTENT_PORT_ATTACHMENT
UNSUPPORTED_STATE_ATTACHMENT
UNSUPPORTED_CONTENT_PORT_ATTACHMENT
INCOMPATIBLE_STATE_OVERRIDES
MIXED_HOST_AFFINITY
```

#### State

```text
INVALID_STATE_PATCH
UNSUPPORTED_STATE_PROPERTY
INVALID_STATE_VALUE
STATE_MOUNTED
STATE_IN_USE
```

#### Content ownership/control

```text
SOURCE_IN_USE
SOURCE_DISPOSED
PORT_MOUNTED
PORT_IN_USE
PORT_DISPOSED
CONNECTOR_NOT_MEMBER
CONNECTOR_DISPOSING
CONNECTOR_DISPOSED
INVALID_FUNNEL
```

#### Payload/annotation

```text
INVALID_UTF8
INVALID_RANGE
STALE_CONTENT_GENERATION
UNKNOWN_ANNOTATION_KIND
INVALID_ANNOTATION_PAYLOAD
PAYLOAD_TOO_LARGE
LIMIT_EXCEEDED
ABI_MISMATCH
```

#### Frame/backend

```text
FRAME_PREPARATION_FAILED
BACKEND_NOT_READY
BACKEND_IO_FAILED
SURFACE_DESYNCHRONIZED
LAYOUT_DID_NOT_CONVERGE
INTERNAL_INVARIANT
```

Exact numeric assignments belong to the generated ABI/error schema and become stable once published.

### 18.3 No mutation on validation failure

Every synchronous validation code guarantees:

- no resource state changed;
- no Source revision assigned;
- no host pending epoch advanced;
- no wake queued;
- no partial registry lease retained.

### 18.4 Connector operational diagnostics

Connector failures are typed separately from host runtime errors. They include:

- Connector/Port/Source IDs in diagnostic-safe form;
- Source revision attempted;
- Funnel kind/config summary;
- offered width/theme generation where relevant;
- retry classification;
- underlying parser/projection code.

They do not expose raw native pointers or unstable occurrence addresses.

---

## 19. Target module responsibilities

Exact file splitting may follow repository conventions. Responsibility boundaries are normative.

### 19.1 TypeScript

```text
runtime/environment
  environment singleton/test injection, lifecycle, artifact handle

runtime/native-resource-registry
  HandleId registration, kind/owner metadata, prepared resolution leases

runtime/wake-broker
  one environment microtask, native drain reports, fairness rearm

runtime/error-channel
  host error storage, listener/reporter, barrier observation

composition / structural transport
  backend-neutral attachment fields, H3 prepare validation, desired commit

state/view-state
  typed public patches, clear semantics, host-bound wrapper lifecycle

state/transport
  generated control calls only; no effect classification

content/source
  environment-owned Source wrapper, append/replace/truncate/dispose

content/funnel
  immutable generated Funnel values, plain and Markdown factories

content/port
  host-owned port wrapper and structural attachment

content/connector
  membership, request/status/disposal API

content/ffi
  same-artifact resolution, ABI handshake, TypedArray encoding, status mapping

content/annotations
  generated semantic unions, byte-range encoder, round-trip fixtures
```

### 19.2 Rust

```text
runtime/environment
  environment generations, Source/host registries, poison state

runtime/resource
  generational registry primitives and prepared leases

runtime/host
  desired vs visible revisions, epochs, pending/blocked state

runtime/scheduler
  pending-host set, wake latch, fair drain protocol

frame/transaction
  candidate storage, capture, commit/abort, failure injection

frame/convergence
  projection/measure/place work loop and diagnostics

state/record
  ViewState storage, base/override/effective values

state/capabilities
  exhaustive node-kind/property support and effect metadata

layout/occurrence_box
  canonical physical box and old Decorated normalization target

content/source
  chunk storage, revisions, snapshots, retention, annotation index

content/registry
  Source ownership, weak subscriptions, connector use counts

content/port
  desired/visible binding and fallback content

content/connector
  requested/committed selection, projection/status/subscription transaction

content/funnel
  plain/Markdown projection plans and cache keys

content/viewport
  read-only context adapters to existing viewport controllers

ffi/content_v1
  exported C symbols, record validation, panic boundary, result/status writes

napi/control
  object/control/query surface backed by the same registries
```

### 19.3 Forbidden dependency edges

```text
structural transport -> state transport           forbidden
structural transport -> content transport         forbidden
Source storage -> Tui-specific Style registry     forbidden
Funnel projection -> JavaScript callback          forbidden in v1
History -> independent Source scheduler            forbidden
TypeScript state API -> native dirty classification forbidden
FFI payload entry -> frame flush/user callback     forbidden
```

Allowed common dependencies include the runtime resource resolver, generated schema types, immutable semantic values, and shared error catalog.

---

## 20. Observability and performance counters

### 20.1 Host/publication

- desired structural revision;
- visible structural revision;
- visible frame revision;
- pending and committed epochs;
- desired revisions superseded before visibility;
- H3 prepare/abort/commit counts and failure codes;
- desired-to-visible latency distribution.

### 20.2 Attachments/state

- HandleId resolutions and lease failures;
- duplicate attachment rejections;
- ViewState mutations accepted/no-op/effective;
- effect classifications by category;
- dirty nodes/roots;
- measured/placed/painted occurrence counts;
- legacy `Decorated` normalization counts by wrapper kind.

### 20.3 Content

- Source bytes/chunks/annotations retained;
- append/replace/truncate operation count and bytes;
- bytes copied versus retained by Arc/chunk reuse;
- Source snapshot count and creation cost;
- subscriber hosts per Source;
- cold/activation-pending/active Connector counts;
- projection cache hit/miss and bytes/rows projected;
- Markdown parse/reuse metrics;
- Connector switch success/failure/fallback counts.

### 20.4 Scheduler/errors

- host pending marks;
- wake latch wins/already-latched counts;
- JS microtasks queued;
- hosts attempted per drain;
- fairness rearm count;
- retry-blocked hosts and unblock cause;
- automatic frame errors by phase/code;
- explicit barrier attempts/failures;
- surface desynchronization/full repaint recovery.

### 20.5 FFI

- calls by symbol;
- text and sidecar bytes per call;
- encoder scratch reuse/copy count where measurable;
- validation failures by status;
- ABI metadata checks;
- artifact path/build ID in debug diagnostics;
- legacy payload calls, which must reach zero before cleanup completion.

Counters must be cheap when disabled and available in tests/benchmarks without parsing log text.

---

## 21. Correctness test matrix

### 21.1 H3 seam

1. Attachment stale/wrong-host/duplicate errors abort H3 prepare and leave composition `projectedOutput`, desired root, and visible frame unchanged.
2. H3 commit succeeds, frame preparation is injected to fail, desired revision advances while old visible frame remains.
3. Retry commits the desired revision without republishing composition.
4. Desired B is superseded by C before visibility; next successful frame shows C and releases B leases.
5. A barrier reports the correct failed epoch/revision and later succeeds after unblock.
6. No ordinary attachment validation occurs in the frame path; failure injection proves the prepare gate owns it.

### 21.2 Handle/host affinity

1. Ordinary unattached View reused across two hosts succeeds.
2. View carrying host A state/port published to host B fails in prepare.
3. DAG reuse of attached subtree twice reports both candidate paths.
4. Same resource old-visible/new-desired remount succeeds without duplicate error.
5. Generational slot reuse never revives an old wrapper.
6. Dispose racing a prepared publication is serialized/blocked correctly.

### 21.3 ViewState

1. Unmounted state accepts schema-valid overrides.
2. Attach to incompatible kind fails without losing overrides.
3. Compatible remount preserves overrides and reveals new base values after clear.
4. `null`, omitted, and clear have distinct tested semantics.
5. Patch validation is atomic.
6. Same effective value creates no frame work.
7. Paint-only change does not measure/place.
8. Dynamic border/padding changes geometry on an initially undecorated node.
9. Old Decorated parity goldens cover nesting/order and damage.

### 21.4 Source lifetime/sharing

1. Source created before any Tui connects to hosts A/B/C and wakes each eligible host exactly once per edge.
2. Disposing host A removes its subscriptions; Source continues feeding B/C.
3. Source outlives all hosts and can feed a later host in the same environment.
4. Cross-environment Source use fails.
5. Source disposal with cold Connector membership fails.
6. Source disposal after all Connectors are gone succeeds while detached immutable history snapshots remain readable.
7. Concurrent mutation model/property tests prove revision linearization and no lock inversion.

### 21.5 Wake broker

Use deterministic race/model tests, and `loom` or equivalent where practical, for:

- mutation before microtask queue;
- mutation during pending-set drain;
- mutation during latch clear/recheck;
- host disposal while weak subscriber token is processed;
- fairness budget exhaustion;
- retry-blocked host plus new epoch;
- no lost wake and no infinite microtask spin.

### 21.6 ContentPort/Connector

1. connect/activate before mount yields waiting-for-mount and no Source subscription.
2. initial mount activates before first visible content frame.
3. unmount retains selection but drops projection/subscription only at frame commit.
4. remount reprojects under new width and preserves viewport control.
5. failed switch keeps old Connector visible and marks new requested Connector failed.
6. source revision/width change retriggers eligible failed candidate without busy loop.
7. deactivation/Connector disposal are transactional.
8. Port and Source disposal codes enforce explicit ownership.

### 21.7 Convergence

1. width change triggers rewrap before final measurement.
2. content measurement changes parent placement and converges.
3. candidate activation participates in the same loop.
4. scroll-only change reuses width-dependent projection.
5. iteration-cycle fixture produces fatal diagnostic with cause trace.
6. Source mutation during frame leaves a later pending epoch.

### 21.8 Annotations

1. every audited Iyon annotation kind/field round-trips through generated sidecar/native storage/projection.
2. multibyte UTF-8 ranges convert and validate correctly.
3. continuous range clips on head truncation.
4. atomic range drops on partial truncation.
5. point anchors obey head semantics.
6. theme changes resolve semantic styles differently per host without Source mutation.
7. one Source with semantic annotations paints correctly in two hosts/themes.
8. malformed kind/flags/payload/range cannot partially mutate Source.

### 21.9 FFI/platform

1. metadata mismatch and wrong build ID fail before payload call.
2. same `.node` path is used by Node-API and `bun:ffi` in dev/stage/package layouts.
3. all supported macOS/Linux platform jobs call the metadata and append symbols.
4. TypedArray lifetime and zero-length pointer cases are covered.
5. unknown status code is treated as ABI failure.
6. no panic unwinds across boundary.
7. explicit size limit returns a recoverable limit error; no recoverable OOM promise exists.

### 21.10 Consumer/deletion

1. Iyon live plain text parity.
2. Iyon streaming Markdown parity.
3. annotation/style parity.
4. History freeze/navigation parity.
5. follow-end/manual scroll parity during append and Connector switch.
6. no legacy payload/registry/scheduler call in integration traces.
7. package exports and source search confirm one runtime content architecture.

---
## 22. Stacked implementation tranches

The work is delivered as eight stacked, individually reviewable merge requests. A later tranche does not weaken an earlier stop gate. Temporary scaffolding is named and deleted by its assigned tranche.

### PERF-13-A — H3/host transaction seam and runtime substrate

#### Goal

Establish the architectural seam before exposing new state/content behavior.

#### Required work

- add explicit desired structural revision separate from visible frame revision;
- add host pending/committed epochs if not already in final form;
- introduce the plane-neutral NativeResourceRegistry/resolver and prepared leases;
- extend backend-neutral H3 nodes with optional branded attachment handles, initially exercised by internal test resources;
- move deterministic attachment/affinity/duplicate/capability validation into H3 prepare;
- make H3 commit install desired structure and mark host pending without layout;
- add frame candidate/commit shell with failure injection and old-visible retry behavior;
- add environment pending-host set, wake latch, TypeScript wake broker, and fair native drain;
- add host runtime error storage/reporter and explicit flush barrier semantics;
- add desired/visible/epoch counters and traces.

#### Must not do

- no public ViewState functionality beyond hidden fixtures;
- no public ContentPort/Source API;
- no content FFI;
- no change to existing visible rendering behavior except through compatibility flush wrappers where required.

#### Temporary allowance

Existing structural/render path may be invoked inside the new candidate shell, but there must be one desired/visible authority. No second host scheduler may be introduced.

#### Stop gates

- H3 prepare failure leaves `projectedOutput`, desired root, and visible frame unchanged;
- injected post-H3 frame failure leaves desired new/visible old and retries without republish;
- desired supersession and lease release tests pass;
- lost-wake race model and no-spin failure test pass;
- automatic drain errors are observable and explicit barriers throw deterministically;
- no ordinary attachment validation remains in the frame path.

### PERF-13-B — ViewState identity, presentation, and occurrence base

#### Goal

Land retained state with the lower-risk presentation path and establish the canonical physical occurrence owner.

#### Required work

- public `tui.viewState()` wrapper, native ViewState registry, HandleId registration, lifecycle/disposal;
- `.state(state)` semantic attachment and H3 host-affinity/duplicate validation;
- exhaustive node-kind capability table infrastructure;
- `OccurrenceBox` base + retained override + effective state representation;
- typed presentation patch/clear/null semantics;
- Rust-side paint-effect classification and damage;
- desired versus visible binding transitions/remount;
- counters and presentation parity tests.

#### Must not do

- TypeScript must not send paint/layout authority flags;
- `.state()` must not create a semantic/stateful wrapper occurrence;
- no geometry field may be advertised until its invalidation path exists.

#### Temporary allowance

Legacy Decorated geometry remains structurally represented. Property-only presentation decorators may normalize into OccurrenceBox under parity tests.

#### Stop gates

- unattached/mounted/remounted/disposed lifecycle tests pass;
- wrong-host and incompatible-kind prepare errors are complete;
- presentation-only updates rebuild no semantic View and perform no measurement;
- base/override/clear behavior passes on remount;
- exhaustive capability match has no wildcard;
- damage goldens match current behavior.

### PERF-13-C — Geometry state, convergence hooks, and Decorated normalization

#### Goal

Move PERF-13 geometry fields to retained occurrence state without structural wrapper creation.

#### Required work

- typed geometry patches/clear semantics;
- native property transition classification for measure/place/paint/projection dependencies;
- dirty propagation and old/new region damage;
- dynamic border-edge/padding/bounds behavior on initially undecorated occurrences;
- audit/classify every current Decorated wrapper;
- normalize every property-only wrapper into canonical base state;
- retain only semantically distinct wrappers with explicit rationale/tests;
- benchmark counters for dirty scope and legacy normalization.

#### Must not do

- no duplicated geometry ownership between old wrapper and OccurrenceBox;
- no blanket whole-tree layout as the permanent implementation;
- no removal of a semantically distinct wrapper without equivalence evidence.

#### Temporary allowance

Semantically distinct legacy wrappers may remain. A debug counter and classification table must enumerate them; unknown/unclassified cases fail tests.

#### Stop gates

- dynamic border/padding changes converge and paint correctly;
- geometry no-op and local-dirty benchmarks meet the agreed regression budget;
- nested decorator call-order parity passes;
- every wrapper is classified; property-only cases have one native owner;
- no structure publication occurs for ViewState mutation.

### PERF-13-D — Content identities, public nouns, and cold control

#### Goal

Land Source/Funnel/Port/Connector identity and lifecycle with no high-volume payload migration yet.

#### Required work

- environment SourceRegistry identity/lifetime skeleton;
- host ContentPort/Connector registries;
- canonical public factories and `port.connect(source, funnel)`;
- ContentPort semantic attachment HandleId, host affinity, duplicate and node-kind checks;
- full desired/visible port binding lifecycle;
- requested versus committed Connector selection/status;
- activate/deactivate/switch/dispose control queue;
- explicit in-use/mounted disposal errors;
- cold membership and weak subscription data structures, initially with synthetic sources;
- owner-death host teardown.

#### Must not do

- no `source.funnel()` object mislabeled as a Funnel;
- no Source tied to the first Tui that uses it;
- no projection for idle/unmounted Connectors;
- no hidden disposal cascades outside host/environment teardown.

#### Temporary allowance

Existing production stream path still renders Iyon. New content entities are exercised by native/unit integration fixtures only.

#### Stop gates

- complete Port/Connector state-machine table tests pass;
- Source outlives host and cross-environment use fails;
- cold Connectors create no subscriptions/projections/wakes;
- failed synthetic switch preserves old committed Connector;
- all disposal/teardown generations and use counts are leak-tested.

### PERF-13-E — Source storage, direct FFI v1, annotations foundation, and multi-host wake

#### Goal

Make Source mutation real, efficient, environment-owned, and able to wake every affected host through one broker.

#### Required work

- chunked retained Source storage, content generation, `u64` revision, snapshots, retention, absolute offsets;
- append/replace/head-truncate operations and explicit limits;
- Source mutex linearization and no-nested-host-lock rule;
- weak active/activation-pending subscriber maintenance;
- one mutation marking N hosts and returning one environment wake bit;
- generated C ABI v1 records/statuses/metadata;
- default-visible symbols exported by the existing `.node`;
- shared native artifact locator and environment-lifetime `dlopen` handle;
- FFI TypeScript encoder/wrapper/status mapping;
- fixed annotation record/payload envelope, even before every consumer kind migrates;
- platform CI proof on the supported matrix.

#### Must not do

- no caller sequence token;
- no host ID in Source FFI identity;
- no FFI callback into JavaScript or flush from the payload call;
- no recoverable OOM status;
- no second shared library unless architecture review reopens the artifact decision after a demonstrated platform failure.

#### Temporary allowance

Production consumers may still call the old text path. Test/debug shadow comparison may compare immutable outputs, but one mutation must not update two authoritative native stores.

#### Stop gates

- same staged `.node` loads through Node-API and `bun:ffi` on every supported target;
- ABI fingerprint mismatch test fails closed;
- shared Source wakes all and only eligible hosts with no JS subscription mirror;
- revision/concurrency/retention/UTF-8 tests pass;
- Source mutation performs no frame work inline;
- FFI throughput/copy metrics meet the agreed baseline and expose regressions.

### PERF-13-F — Connector projection, unified convergence, and viewport integration

#### Goal

Make new content visible through the normative activation/projection/layout transaction.

#### Required work

- plain-text Funnel and projection cache key;
- Source snapshot acquisition in candidate frame;
- requested Connector activation inside the convergence loop;
- width-dependent projection before/during measurement;
- measurement/place feedback and defensive convergence diagnostics;
- old-Connector/empty fallback on operational candidate failure;
- atomic projection/subscription/status commit;
- ContentPort allocation/clip binding;
- read-only ScrollPane/RowViewport context and extent/clamp/follow integration;
- scroll-only visible-window reuse;
- Connector status/error delivery after commit.

#### Must not do

- no post-layout activation phase;
- no Connector-owned scroll offset/follow state;
- no Source mutation during projection;
- no JS callback Funnel.

#### Temporary allowance

Iyon production may still use the old path while new plain-text fixtures run end-to-end. A controlled parity harness may render old and new off-screen outputs for comparison.

#### Stop gates

- activation, switch failure fallback, unmount/remount, and source-during-frame tests pass;
- width/content/layout feedback converges deterministically;
- scroll-only updates avoid full rewrap/reparse;
- connector switches preserve viewport intent;
- frame candidate abort leaks no projection/subscription/binding;
- plain-text visual parity goldens pass.

### PERF-13-G — Complete annotations, Markdown, History, and Iyon migration

#### Goal

Move the real consumer and every currently required semantic feature to the new content plane.

#### Required work

- freeze the current Iyon annotation declaration audit;
- generate exact TypeScript/wire/Rust schemas for every kind and field;
- semantic host-independent style descriptors and per-host/theme resolution;
- head-truncation policy table and round-trip fixtures;
- MarkdownFunnel using new Source snapshots/projection/convergence;
- migrate Iyon live content creation/mutation and Connector control;
- migrate History freeze/navigation to committed snapshots/projections;
- migrate follow-end/manual scroll behavior through the viewport owner;
- adapt `TextStream.update()` directly to Source.replace where public compatibility is retained;
- expose legacy-use counters and deprecation diagnostics.

#### Must not do

- no host-native Style IDs in Source annotations;
- no separate Markdown or History content scheduler;
- no consumer-specific bypass around ContentPort/Connector;
- no silent annotation field loss.

#### Temporary allowance

Old production path may remain behind a short-lived fallback feature flag for comparison/rollback during this tranche only. The flag chooses one architecture at startup; it never double-dispatches one mutation.

#### Stop gates

- every audited Iyon annotation kind/field round-trips and paints with parity;
- Markdown streaming/history goldens pass;
- one Source renders correctly in two hosts/themes;
- History retains/frees snapshots according to policy;
- Iyon default integration uses the new path;
- legacy invocation counter is zero in the complete consumer suite.

### PERF-13-H — Deletion, platform/lifetime hardening, and performance acceptance

#### Goal

Remove superseded architecture and prove the final system is singular, portable, bounded, and fast.

#### Required work

- delete old high-volume payload bridge, native registries, queues, scheduler hooks, and production feature flag;
- remove redundant View/text rebuilding paths;
- retain only pure compatibility adapters/internal algorithms documented in §16;
- complete all host/environment/GC/disposal stress tests;
- run platform package smoke tests from produced artifacts;
- run performance/memory suite and inspect counters;
- update API-H3/PERF-13 docs and package exports;
- record final code-search and symbol-audit evidence.

#### Must not do

- no indefinite “temporary” dual path;
- no disabled-by-default legacy scheduler/registry left in production source;
- no completion claim with unsupported platform job skipped;
- no counter regression hidden by averaging unrelated workloads.

#### Final acceptance gates

- one public/runtime high-volume content architecture exists;
- all supported platform FFI jobs pass against packaged artifacts;
- no lost wake, lifetime leak, stale generation, or old/new partial commit in stress/model tests;
- state/content mutations do not rebuild semantic structure;
- cold Connectors consume no projection/wake work;
- performance and memory acceptance table is signed off with raw counter output;
- Iyon uses the new default path with behavior parity;
- all completion criteria in §24 are checked.

---

## 23. Benchmark and acceptance plan

### 23.1 Baselines

Capture before PERF-13 consumer migration:

- structural render/update benchmark suite at the final API-H3 commit;
- current Iyon live assistant-text append workload;
- current Markdown/history freeze workload;
- memory after long retained stream/head truncation;
- idle-frame CPU and terminal write counts;
- package startup/native-load time.

Record commit, platform, Bun/Rust versions, build profile, terminal backend, corpus, and raw counters.

### 23.2 State workloads

- presentation toggle on one leaf in 2k and 10k occurrence trees;
- geometry border/padding toggle on leaf/container;
- repeated no-op patch;
- remount state between compatible nodes;
- theme/selector repaint with conservative versus indexed propagation;
- old Decorated parity path versus normalized base.

Required evidence is not one wall-clock number. Report semantic View rebuild count, measured/placed/painted occurrences, damage cells/rectangles, allocations, and elapsed time.

### 23.3 Source/FFI workloads

- append 1, 16, 128, 4 KiB, and 64 KiB payloads;
- append with zero/few/many annotations;
- replace small and retained-large Sources;
- head retention over millions of appended bytes;
- one Source with one host and with 2/4/8 hosts;
- many cold Connectors versus one active Connector;
- FFI encoding/call only and end-to-end frame separately.

Report bytes encoded/copied, Source lock time, subscriber fan-out time, wake microtasks, snapshots, chunks, projections, and frame work.

### 23.4 Projection/convergence workloads

- plain unwrapped/wrapped text at stable/changing width;
- Markdown incremental append;
- scroll-only movement;
- follow-end append;
- Connector switch same/different Source/Funnel;
- candidate projection failure fallback;
- content-driven parent measurement with one/two convergence iterations.

### 23.5 Memory/lifetime workloads

- create/dispose many hosts while one Source survives;
- repeated Port/Connector creation/disposal;
- desired revisions superseded before visibility;
- failed frames retaining old visible/candidate structures;
- History freeze/release;
- head truncation with annotation clipping;
- GC of wrappers before/after explicit owner teardown.

### 23.6 Acceptance philosophy

Exact thresholds are benchmark-owned and may be tuned. Architectural acceptance is fixed:

- no O(total semantic tree) work for a local paint-only mutation absent a documented conservative dependency;
- no full Source copy per append/frame snapshot;
- no projection for cold Connectors;
- no JS per-host fan-out graph for Source mutation;
- no more than one queued environment drain microtask per edge;
- no old high-volume bridge in the final path;
- no material regression in API-H3 structural workloads after accounting for the new attachment fields when unused.

---

## 24. Definition of done

PERF-13 is complete only when every statement below is true.

### 24.1 Structural/H3

- desired structural state and visible frame state are explicit and separately revisioned;
- all permanent attachment/affinity/capability errors occur in H3 prepare;
- H3 commit is infallible authoritative desired publication;
- frame failure never rolls semantic composition back;
- barriers and automatic error delivery have deterministic semantics;
- semantic Views contain only backend-neutral HandleIds.

### 24.2 Resource boundaries

- structural/state/content/component transports share a plane-neutral resolver;
- structural transport imports neither state nor content dispatcher;
- generational liveness and host/environment ownership are enforced;
- View host affinity is documented and tested;
- desired/visible attachment transitions and disposal are leak-free.

### 24.3 Retained state

- every physical state-capable occurrence has canonical base box state;
- ViewState patches remain outside semantic Views;
- Rust owns exhaustive capability/effect metadata;
- null/clear/remount semantics are exact;
- dynamic border/padding works without structural wrapper creation;
- every old Decorated case is normalized or explicitly retained for distinct semantics;
- local state mutation uses bounded dirty/damage work evidenced by counters.

### 24.4 Content identities/lifetimes

- Sources are environment-owned and can outlive/share across hosts;
- Ports/Connectors are host-owned and explicitly host-affine;
- canonical API nouns match the entity model;
- Port mount/unmount/remount and Connector request/commit/failure/disposal states are complete;
- individual disposal is explicit and owner teardown is safe;
- inactive/unmounted Connectors are cold.

### 24.5 Scheduling/transaction

- Source mutation can wake every affected host through one environment broker;
- native epochs/pending-host state are authoritative;
- no lost wakes or microtask retry spin exists;
- Connector activation/projection participates in one convergence loop;
- source/state/control/layout/paint commit as one logical frame;
- failed Connector switch preserves old visible content;
- automatic errors are stored/reported and explicit barriers surface them.

### 24.6 Annotations/viewport/consumer

- every current Iyon annotation kind/field is represented by the generated sidecar and native semantic model;
- Source annotations are host-independent and head truncation is kind-correct;
- ScrollPane/RowViewport owns offset/follow state;
- Connector receives only read-only viewport context;
- Markdown is a Funnel;
- History consumes committed new-path snapshots/projections;
- Iyon parity and lifetime tests pass.

### 24.7 FFI/platform

- high-volume payloads use mandatory direct FFI;
- both bindings load one staged `.node` through one locator;
- metadata/fingerprint checks fail closed;
- native Source revision is the linearization token;
- no caller sequence lanes or recoverable OOM claim remain;
- every supported staged/published platform job passes.

### 24.8 Cleanup

- no second public/runtime high-volume content architecture remains;
- any `TextStream.update` compatibility is a pure `Source.replace` adapter;
- old registries/queues/payload symbols/schedulers are deleted;
- legacy counters are zero and then removed or converted to assertions;
- final documentation references the final API-H3 SHA and this handoff.

---

---

# Part I appendices

## Appendix A — Audited baselines

The final artifact generator inserts the exact commits, dates, subjects, and document hash below. API-H3 is the architectural incoming baseline. The Iyon commit is a consumer-audit snapshot, not a claim that PERF-13 must pin Iyon to that branch forever.

| Artifact/repository | Branch | Commit/hash | Commit date | Subject/role |
|---|---|---|---|---|
| Attached PERF-13 baseline | — | SHA-256 `62c16f30718e314f8b61b78cd4ba489fc58f7a9d59615a1f707de77ca712f4bf` | — | Integrated verbatim in Part II |
| `alexykn/iyon-tui` | `api-h3-composition-transport-seam` | unavailable in research environment | — | API-H3 implementation baseline |
| `alexykn/iyon` consumer audit | — | unavailable in research environment | — | Consumer audit paths must be refreshed in PERF-13-G |

Any stale branch or SHA reference in the integrated baseline body is subordinate to this table.

## Appendix B — Open-question closure map

| Original open issue | Normative resolution |
|---|---|
| 1. H3 commit vs frame commit | §§3 and 6: H3 commits desired structure; frame commits visible state; permanent checks in H3 prepare; flush is visibility barrier. |
| 2. attachment identity in SemanticViewNode | §4.1: branded framework HandleId only. |
| 3. structural resolution dependency | §§4.2–4.3: plane-neutral NativeResourceRegistry/resolver and prepared leases. |
| 4. Source outliving Tui | §§2 and 12: Source is environment-owned; host resources are separate. |
| 5. shared Source wake | §5: one native pending-host set and one environment JS wake broker. |
| 6. automatic flush errors | §5.7–§5.10: structured host error channel; no microtask throw; synchronous barrier throws. |
| 7. activation/layout contradiction | §6: one projection/measurement/placement convergence loop. |
| 8. stateful occurrence with Decorated | §8: underlying physical occurrence always owns OccurrenceBox; compatibility normalization. |
| 9. node-kind support and typing | §7: TS shapes + exhaustive Rust capability table; precise null/clear/remount rules. |
| 10. Port lifecycle | §§10–11: desired/visible mount and requested/committed Connector state machines, explicit disposal. |
| 11. Source/Funnel/connect API mismatch | §9: immutable Source-neutral Funnel; canonical `port.connect(source, funnel)`. |
| 12. View/Port host affinity | §§4.5 and 9.4: attachment-derived host affinity validated in H3 prepare. |
| 13. annotation migration | §13 plus repository audit: semantic host-independent annotations, exact generated sidecar, kind-specific truncation. |
| 14. old stream paths | §16: algorithms may remain; second runtime/public architecture may not. Markdown and History migrate. |
| 15. Port/viewport seam | §14: viewport controller owns offset/follow; Port owns allocation/clip only. |
| 16. FFI ordering token | §§12.2 and 15.10: native Source lock/revision; no caller sequence. |
| 17. bun:ffi artifact | §§15.2–15.4 and 15.14: same staged `.node`, one locator, metadata handshake, platform gate. |
| 18. OUT_OF_MEMORY | §15.12: removed as recoverable status; limits are recoverable, allocator exhaustion is fatal. |
| missing implementation tranches | §22: eight stacked MRs with bounded dual paths and stop gates. |

## Appendix C — Critical event timelines

### C.1 Valid publication followed by retryable frame failure

```text
JS composition prepares View B
  -> H3 resolves attachments and validates B
  -> H3 commit installs desired revision B / epoch 41
  -> composition projectedOutput becomes B
  -> broker attempts frame 41
  -> backend preparation fails retryably
  -> visible frame remains A / committed epoch 40
  -> error channel records (desired B, attempted 41)
  -> later readiness signal or tui.flush()
  -> frame for latest desired B commits
  -> visible frame becomes B / committed epoch 41
```

### C.2 Desired revision superseded before visibility

```text
visible A
H3 commits desired B / epoch 41
frame B fails
H3 commits desired C / epoch 42
B desired leases released where safe
frame C succeeds
visible C / committed epoch 42
B need never become visible
```

### C.3 Shared Source append

```text
Source S mutation lock
  -> append bytes + annotations
  -> assign revision R+1
  -> snapshot weak eligible subscribers [A, B, C]
release Source lock
  -> mark host A pending
  -> mark host B pending
  -> mark host C pending
  -> first edge sets environment wake latch
FFI returns one SCHEDULE_ENVIRONMENT_DRAIN flag
JS queues one microtask
native fair drain attempts A/B/C
```

### C.4 Connector switch failure

```text
Port P visible Connector A
B.activate() -> requested B
candidate frame snapshots/projects B
B projection fails operationally
candidate falls back to A projection
frame may commit unrelated structure/state changes
A remains visible + subscribed
B status = failed, requested = true, visible = false
new eligible input or retry requests another attempt
```

### C.5 Port unmount/remount

```text
visible occurrence X attaches P with Connector A
H3 desired root removes P
until frame commit: X/A remain visible and subscribed
unmount frame commits: projection/subscription dropped; selection retained
P = unmounted; A = waiting-for-mount/cold
H3 desired root attaches P at Y
candidate projects A for Y width/viewport
commit: Y/A visible and subscribed
```

## Appendix D — Locking and lifetime invariants

1. Source mutation lock is never held while acquiring a host frame lock.
2. Environment registry lock is not held through Funnel projection/layout/user reporting.
3. Prepared H3 resource leases pin generational resources until commit/abort.
4. Desired and visible leases may coexist for one resource during one remount.
5. Connector Source use count outlives cold membership and ends only on Connector destruction.
6. Source snapshots hold immutable chunk/annotation Arcs, not the mutable Source lock.
7. Frame candidate subscriptions are provisional and become authoritative only at commit.
8. Old visible subscriptions are removed only in the same commit that replaces/drops their projection.
9. Host disposal removes subscriber records before invalidating host generation or makes stale generations harmless.
10. FFI never retains JS pointers or calls JS while native locks are held.
11. Error listeners run after native locks and frame transaction scopes are released.
12. No panic/unwind crosses FFI or Node-API.

## Appendix E — Primary technical references

These references support feasibility and boundary choices; repository behavior and this handoff remain the project-specific authority.

- Bun FFI (`dlopen`, symbols, pointers, TypedArrays): https://bun.sh/docs/runtime/ffi
- Bun Node-API compatibility/runtime integration: https://bun.sh/docs/runtime/node-api
- Node-API thread-safe functions for a future native-thread wake notifier: https://nodejs.org/api/n-api.html#asynchronous-thread-safe-function-calls
- Rust `Vec::try_reserve` for deliberately fallible capacity growth: https://doc.rust-lang.org/std/vec/struct.Vec.html#method.try_reserve
- Rust allocation-error handler semantics: https://doc.rust-lang.org/std/alloc/fn.handle_alloc_error.html
- Rust FFI and unwinding constraints: https://doc.rust-lang.org/nomicon/ffi.html#ffi-and-unwinding
- WHATWG Encoding Standard / `TextEncoder`: https://encoding.spec.whatwg.org/#interface-textencoder

## Appendix F — Same-artifact-format local proof

# Local same-artifact format proof

Date: 2026-08-29T17:56:31Z
Platform: Linux 6.18.35 x86_64
Result: NOT RUN (cc or bun unavailable)


This probe establishes only that a default-visible C symbol in a shared object carrying a `.node` suffix is callable through `bun:ffi` in the recorded environment. The real staged-addon/platform matrix in PERF-13-E remains the merge gate.

## Appendix G — Repository path audit

The path lists below were generated at the audited commits. They are navigation evidence for implementation agents and a guard against designing from stale recollection.

### G.1 iyon-tui

# iyon-tui path audit

Repository unavailable.


### G.2 Iyon consumer

# iyon path audit

Repository unavailable.


## Appendix H — Annotation declaration/consumer index

# Annotation symbol index at audited commits

This is a navigation index, not a second specification. The generated annotation schema must account for each relevant declaration and consumer before PERF-13-G passes.

## iyon-tui

- Repository unavailable.
## iyon

- Repository unavailable.


The index is intentionally a navigation list rather than copied implementation source. PERF-13-G freezes these declarations into the generated annotation schema and adds a checked migration table. Every relevant current field must be accounted for as preserved, intentionally translated, or explicitly removed by a separate API decision; omission is a test failure.

---

# Part II — Integrated PERF-13 baseline specification

> This is the complete earlier PERF-13 handoff, included so implementation agents retain every already-settled constraint. Part I resolves and supersedes the seam questions listed in Appendix B. Where wording conflicts, Part I wins. The audited API-H3 commit in Appendix A replaces stale API-H1 baseline references.

## PERF-13 — Three-Plane Retained Runtime

**Repository:** `alexykn/iyon-tui`  
**Baseline inspected:** `api-h3-composition-transport-seam` at `dc928ba1dc3c209ebadc1c2aa25398275f726c1b`  
**Required sequence:** `PERF-12 → API-H3 → API-H2 / STRUCT-1 → API-H3 → PERF-13`  
**Document type:** normative architecture and implementation handoff  
**Audience:** implementation agent with competent Rust/TypeScript skills but no assumed prior knowledge of retained UI internals  
**Delivery model:** stacked, individually reviewable merge requests; do not merge any tranche to the protected integration branch until every PERF-13 tranche is complete and the final gates pass

---

### 0. Executive directive

PERF-13 has one architectural thesis:

> **The retained semantic View DAG stops being the universal carrier for structure, mutable runtime state, and content.**

The runtime must establish three distinct planes:

```text
                         TYPESCRIPT

                 structure / composition
                          │
                          ▼
                ┌────────────────────┐
                │ STRUCTURAL PLANE   │
                │ retained View DAG  │
                └─────────┬──────────┘
                          │ stable semantic identity
                          │
               ┌──────────┴───────────┐
               │                      │
               ▼                      ▼

      RETAINED STATE PLANE        CONTENT PLANE
      ────────────────────        ─────────────

      geometry                    ContentPort
      presentation                    │
      style-state / interaction       │ 0..N Connectors
               │                      │ 0..1 active
               │                   Funnels
               │                      │
               │                   Sources
               │                      │
               └──────────┬───────────┘
                          ▼

                 ┌──────────────────┐
                 │   RUST RUNTIME   │
                 │                  │
                 │ measurement      │
                 │ layout           │
                 │ projection       │
                 │ paint            │
                 │ damage           │
                 │ terminal backend │
                 └──────────────────┘
```

The governing rules are:

1. **If structural topology and retained attachment identity did not change, the structural View DAG should normally not change.**
2. **TypeScript communicates semantic intent; Rust classifies and executes the consequences.**
3. **A semantic `ViewId`/`NodeId` is not a mutable occurrence address.** A shared View may occur more than once. Dynamic state and content therefore require explicit opaque attachment identities.
4. **State and content mutation do not execute composition scopes.** They bypass `defineView`, semantic child ownership, and structural lowering.
5. **The committed frame remains authoritative until a complete replacement frame has been validated, prepared, and committed.**
6. **Bulk content bytes use one mandatory fast data plane.** N-API remains the lifecycle/control plane; it is not a permanent second payload architecture.
7. **PERF-13 implements cold connector standby only.** It does not implement buffered or hot inactive connectors, arbitration, priority, preemption, or automatic fallback scheduling.

The scope correction is precise:

```text
PERF-13 includes:
    structural plane
    retained mutable state plane
    retained content plane
    multiple connectors per port
    explicit manual activation
    cold inactive semantics

PERF-13 excludes:
    buffered inactive delivery
    hot inactive projection/layout
    automatic arbitration
    priority/preemption/yield
    Kitty/Sixel/video/live surfaces
    property-level reactive bindings
```

#### 0.1 Required precondition: API-H3

PERF-13 must not start until API-H3 has established this invariant:

> **Composition owns semantic retention. Structural transport owns physical/native retention.**

After API-H3:

```text
composition/
    semantic View identity
    scope execution
    State dependency tracking
    child occurrence ownership
    semantic subtree reuse
          │
          │ narrow structural publication seam
          ▼
transport/structural/
    NativeRef mapping
    leases
    bridge records
    generated structural ABI
    materialization
    N-API/direct structural lowering
```

Composition must not import or name:

```text
NativeRef
bridge IR records
View ABI calls
materialization policies
transport generations
state-plane operations
content-plane operations
```

State and content APIs must route from their runtime/API owners directly to their respective transports. Composition must never become the dispatcher for all native work.

If API-H3 leaves composition importing structural transport implementation modules, stop. Do not use PERF-13 to hide that debt behind new abstractions.

#### 0.2 What “done” means

PERF-13 is complete only when all of the following are true:

- A paint-only state change performs no semantic View construction, no new `NodeId`, no composition scope execution, and no structural publication.
- A geometry state change performs no semantic View reconstruction and propagates native work only through the required dependency frontier.
- High-frequency text append/replace performs no View construction and sends bytes through the content data ABI, not the View ABI.
- A port can retain several connectors and switch explicitly between them without changing the structural root or rerunning parent composition.
- Inactive cold connectors perform no projection, wrapping, layout, paint, or connector-local buffering.
- A failed state commit or connector activation leaves the previous committed frame and previous active connector visible.
- Existing PERF-12 structural identity, direct/N-API parity, leases, retry behavior, and performance gates do not regress.
- Old high-volume View/text payload paths are deleted after migration; there is one structural architecture and one content architecture.

---

### 1. Normative language and decision policy

The words **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** are normative.

This handoff deliberately resolves the architectural questions that were previously open. An implementation may change an internal representation only when all of these conditions hold:

1. the public and cross-plane semantics in this document remain unchanged;
2. the replacement is documented in the tranche completion report;
3. differential correctness tests pass;
4. the replacement does not broaden tranche scope;
5. the replacement is approved during review before later tranches depend on it.

Public type and method names in examples are **normative working names**. A naming-only API review may adjust spelling before the first public PERF-13 tranche is finalized, but it must not alter identity, ownership, lifecycle, scheduling, or failure semantics.

No `TODO`, unspecified fallback, or “implement whichever is easiest” decision is acceptable in the completed implementation.

---

### 2. Baseline reality at `api-h3-composition-transport-seam`

The design must be implemented against the code that exists, not against an imagined clean architecture.

#### 2.1 TypeScript View currently crosses too many layers

At the inspected baseline, [`values/view.ts`](https://github.com/alexykn/iyon-tui/blob/dc928ba1dc3c209ebadc1c2aa25398275f726c1b/packages/iyon-tui/src/values/view.ts) imports or participates in:

```text
semantic View construction
bridge IR records
NodeId/native path metadata
retained composition helpers
style lowering
PersistentSeq structural derivations
native fast-path decisions
```

That coupling is why API-H2 makes ownership visible and API-H3 creates the semantic publication seam. PERF-13 must not add state/content lowering to the same module.

#### 2.2 Semantic execution is already transactional

[`execution.ts`](https://github.com/alexykn/iyon-tui/blob/dc928ba1dc3c209ebadc1c2aa25398275f726c1b/packages/iyon-tui/src/execution.ts) already provides important correctness patterns:

```text
prepare all publications
commit once
abort without publishing partial roots
restore retry obligations
microtask-coalesce State invalidations
keep execution-scope identity distinct from NodeId/NativeRef
```

PERF-13 should reuse the model—authoritative committed state plus staged work—but state/content mutation must not be implemented as fake composition scopes.

#### 2.3 Semantic and physical retention are currently adjacent

[`retained_dag.ts`](https://github.com/alexykn/iyon-tui/blob/dc928ba1dc3c209ebadc1c2aa25398275f726c1b/packages/iyon-tui/src/retained_dag.ts) owns JS semantic-node-to-`NativeRef` correspondence, generation-scoped hints, leases, transaction-local materialization, and cold recovery. [`native_view_abi.ts`](https://github.com/alexykn/iyon-tui/blob/dc928ba1dc3c209ebadc1c2aa25398275f726c1b/packages/iyon-tui/src/native_view_abi.ts) owns generated structural calls and retained structural edit transactions.

Those are physical/native-retention responsibilities. API-H3 must isolate them before PERF-13 adds independent state and content transports.

#### 2.4 A semantic View is not a unique mounted occurrence

The Rust presentation IR stores immutable semantic Views behind `Arc<ViewNode>`, with a semantic `ViewId`. Layout expands those semantic values into occurrence-specific `LayoutNode`s with:

```text
rect
content_rect
clip_rect
parent
children
component association
style context
```

The same semantic View can appear at more than one occurrence. Therefore:

> **Never target mutable state by `ViewId`, TypeScript `NodeId`, or `NativeRef`.**

`ViewId` remains useful as an immutable semantic/cache identity. It is not sufficient to identify “the box currently at row 7, column 12 under this parent.”

#### 2.5 Current View properties are immutable semantic fields

The baseline Rust `ViewNode` contains immutable fields such as:

```text
width/height rule
decoration
style states/facts
ViewKind
```

`Decoration` currently includes padding, bounds, surface background, border, and text style. Fluent changes construct a new semantic View identity. PERF-13 migrates selected properties to retained native state without claiming every property is mutable in the first release.

#### 2.6 The current border model is one terminal-cell edge, not arbitrary thickness

The current TypeScript [`BorderSpec`](https://github.com/alexykn/iyon-tui/blob/dc928ba1dc3c209ebadc1c2aa25398275f726c1b/packages/iyon-tui/src/types.ts) has glyph/style/edge/color semantics and no border-width property. PERF-13 must not invent `borderWidth = 3` as a shipped API merely because it is a useful conceptual example.

For PERF-13:

```text
border absent       → no border inset
border edge present → exactly one terminal cell on that edge
```

Arbitrary terminal border thickness is future work.

#### 2.7 The native stream subsystem is not greenfield

The existing Rust stream modules already provide:

- width-independent snapshots;
- monotonic source revisions and absolute UTF-8 byte coordinates;
- stable frontiers;
- source compaction validation;
- projected semantic text;
- width-specific compilation and row indexing;
- pure History transfer planning;
- follow-end/detached viewport anchors.

Relevant baseline modules include:

- [`stream/model.rs`](https://github.com/alexykn/iyon-tui/blob/dc928ba1dc3c209ebadc1c2aa25398275f726c1b/crates/iyon-tui/src/stream/model.rs)
- [`stream/source.rs`](https://github.com/alexykn/iyon-tui/blob/dc928ba1dc3c209ebadc1c2aa25398275f726c1b/crates/iyon-tui/src/stream/source.rs)
- [`stream/projected.rs`](https://github.com/alexykn/iyon-tui/blob/dc928ba1dc3c209ebadc1c2aa25398275f726c1b/crates/iyon-tui/src/stream/projected.rs)
- [`stream/snapshot.rs`](https://github.com/alexykn/iyon-tui/blob/dc928ba1dc3c209ebadc1c2aa25398275f726c1b/crates/iyon-tui/src/stream/snapshot.rs)
- [`stream/transfer.rs`](https://github.com/alexykn/iyon-tui/blob/dc928ba1dc3c209ebadc1c2aa25398275f726c1b/crates/iyon-tui/src/stream/transfer.rs)
- [`stream/pane`](https://github.com/alexykn/iyon-tui/tree/dc928ba1dc3c209ebadc1c2aa25398275f726c1b/crates/iyon-tui/src/stream/pane)

PERF-13 must refactor and reuse this machinery. It must not replace it with an unindexed `String` plus repeated full wrapping.

#### 2.8 The host already has retained local invalidation machinery

The current [`SceneHost`](https://github.com/alexykn/iyon-tui/blob/dc928ba1dc3c209ebadc1c2aa25398275f726c1b/crates/iyon-tui/src/scene/host.rs) retains:

```text
stable resolved scene
layout cache
paint cache
last surface
mount graph
focus/capabilities
component-local invalidation sets
```

It already attempts same-shape component subtree patching, local paint into the retained surface, and full-layout fallback when topology or geometry changes. PERF-13 should generalize these ideas from “component changed” to “attached node state/content changed.” It must not create a parallel renderer.

#### 2.9 Current stream mutation renders too eagerly

The baseline native host updates a `HostTextStream`, invalidates the frame, and calls `advance_and_render()` from each mutation. That is correct but defeats high-frequency append coalescing.

PERF-13 must change the scheduling contract so that:

```text
append/update/state mutation
    → mutate or enqueue cheap native semantic state
    → advance one pending-work epoch
    → schedule one host flush
    → project/layout/paint once for the latest state
```

#### 2.10 Existing performance counters are the starting point

[`perf.rs`](https://github.com/alexykn/iyon-tui/blob/dc928ba1dc3c209ebadc1c2aa25398275f726c1b/crates/iyon-tui/src/perf.rs) already measures View construction/copying, N-API cache behavior, resolve, measure, prepare, layout emission, paint, compositing, History, stream reindex/reuse, and PersistentSeq work.

PERF-13 must extend this seam rather than invent an unrelated benchmark reporting format.

---

### 3. Research synthesis: patterns to adopt and patterns not to copy

PERF-13 borrows proven separation principles from other domains. It does not attempt to clone those systems wholesale.

#### 3.1 Retained UI engines: dirty effects belong to the renderer

Flutter’s [`markNeedsLayout`](https://api.flutter.dev/flutter/rendering/RenderObject/markNeedsLayout.html) marks layout dirty, propagates to a relayout boundary or parent according to dependency, schedules a visual update, and coalesces sequential writes. [`markNeedsPaint`](https://api.flutter.dev/flutter/rendering/RenderObject/markNeedsPaint.html) separately schedules paint and can stop at repaint boundaries. Flutter also records when intrinsic/baseline queries create dependencies that cross ordinary boundaries.

Qt Quick’s [`QSGNode::DirtyStateBit`](https://doc.qt.io/qt-6/qsgnode.html) distinguishes matrix, node-add/remove, geometry, material, opacity, and other dirty categories on a retained render graph.

Adopt:

```text
semantic property mutation
    → native effect classification
    → typed dirty flags
    → dependency-aware propagation
    → batched frame processing
```

Do not copy:

```text
Flutter's object hierarchy
Qt's rendering-thread API surface
GPU-specific layer machinery
```

#### 3.2 Compositors/display stacks: pending state must commit atomically

Wayland `wl_surface` requests modify pending state. [`wl_surface.commit`](https://wayland.freedesktop.org/docs/html/apa.html#protocol-spec-wl_surface) makes buffer, damage, input/opaque region, and related state current together. Pending damage is accumulated as a union and only interpreted at commit.

Linux DRM/KMS performs an atomic check before commit and requires failure-prone preparation before the visible hardware flip. See the current [DRM KMS atomic documentation](https://docs.kernel.org/gpu/drm-kms.html).

Adopt:

```text
current committed runtime state
pending mutation batch
validate/prepare off to the side
atomic swap on success
old frame remains authoritative on failure
old connector remains active until candidate is ready
```

Do not copy:

```text
buffer-release protocol details
GPU fences/page-flip semantics
subsurface synchronization rules
```

#### 3.3 Media graphs: contract, link, policy, and bytes are different things

GStreamer separates media-format negotiation from allocation negotiation. Its [caps negotiation](https://gstreamer.freedesktop.org/documentation/plugin-development/advanced/negotiation.html) uses explicit capabilities, acceptance checks, fixed/transform/dynamic cases, and reconfiguration. Its [buffer-pool design](https://gstreamer.freedesktop.org/documentation/additional/design/bufferpool.html) negotiates allocation only after format compatibility.

PipeWire exposes nodes, ports, and links as separate graph objects, while its session manager owns higher-level routing policy. Its [overview](https://docs.pipewire.org/page_overview.html) also distinguishes passive links that do not make a graph active. Port implementation states move through creation, configuration, readiness, and paused phases.

Adopt:

```text
Source            authoritative semantic data
Funnel            typed transformation/delivery contract
Connector         one retained link with attachment-local state
ContentPort       one structural receiving region
manual activation policy outside the data object
semantic/backend/geometry checks at distinct phases
```

Do not copy:

```text
a generic multimedia pipeline scheduler
arbitrary graph cycles
buffer-pool negotiation for text in PERF-13
background inactive processing
```

#### 3.4 SIP/SDP/RTP: control, description, and payload may share a feature but not a transport

SIP establishes/modifies sessions; SDP describes and negotiates media parameters; RTP transports media. [RFC 3261](https://www.rfc-editor.org/rfc/rfc3261), [RFC 3264](https://www.rfc-editor.org/rfc/rfc3264), and [RFC 3550](https://www.rfc-editor.org/rfc/rfc3550) make those roles explicit.

The transferable lesson is not SIP message syntax. It is this separation:

```text
N-API control/lifecycle
    ≠
Funnel/Port compatibility description
    ≠
fast content payload transport
```

#### 3.5 ECS/game engines: stale identities need generations; writes should not imply real change

Bevy entities use an index plus generation so a freed slot does not make an old identifier valid for a new entity. Its [entity lifecycle documentation](https://docs.rs/bevy_ecs/latest/bevy_ecs/entity/index.html) explicitly bumps generations when slots are freed. Bevy change detection also demonstrates both the usefulness and the danger of mutation ticks: a mutable dereference may mark changed even when the value is equal, while helpers such as `set_if_neq` avoid redundant work.

Adopt:

```text
typed generational handles
fallible stale-handle validation
equality check before advancing property revisions
monotonic dirty/revision generations
```

Do not copy:

```text
a complete ECS
archetype storage
system scheduling
implicit “any mutable borrow means changed” semantics
```

#### 3.6 Native transports: stable control, isolated experimental fast lane

Node-API is documented as ABI-stable and provides environment-scoped instance data and teardown. See [Node-API](https://nodejs.org/api/n-api.html). Bun FFI can pass TypedArray-backed pointers, but Bun warns that raw pointers are unsafe and its FFI surface must be treated as a platform-specific capability. See [Bun FFI](https://bun.sh/docs/runtime/ffi).

Adopt:

```text
Node-API for opaque objects, lifecycle, configuration, errors, and queries
one isolated direct-FFI adapter for bulk bytes
synchronous copy into Rust-owned memory before FFI return
startup ABI/version probes
no raw pointer retained across the call
```

The FFI lane is mandatory for production content payloads, but it must be isolated behind one module and gated on the exact runtime/platform matrix already supported by the project.

---

### 4. Architectural ownership after API-H2 and API-H3

#### 4.1 TypeScript ownership

The expected ownership is:

```text
src/
├── api/
│   ├── view/
│   │   ├── ... existing structural semantics
│   │   └── retained-state.ts       # public ViewState semantics
│   ├── presentation/
│   ├── content/
│   │   └── retained-content.ts     # Sources, Funnels, Ports, Connectors
│   └── controls/
│
├── composition/
│   └── ... semantic retained execution only
│
├── runtime/
│   ├── ... Tui host/lifetime
│   ├── retained-handles.ts         # host binding and disposal orchestration
│   └── flush-scheduler.ts          # one host wake/flush scheduler
│
├── transport/
│   ├── structural/
│   ├── state/
│   │   └── control.ts              # N-API state mutation/lifecycle
│   ├── content/
│   │   ├── control.ts              # N-API content lifecycle/activation
│   │   ├── data.ts                 # semantic payload facade
│   │   └── ffi.ts                  # the only Bun FFI implementation
│   ├── native/
│   └── abi/
│       ├── structural/
│       ├── state/                  # only if generated schema adds value
│       └── content/                # control/data definitions may differ
│
└── testing/
```

This is responsibility guidance, not a demand for one file per symbol. Keep cohesive families together.

Import direction:

```text
api/state/content
      │
      ▼
runtime handle/lifetime facade
      │
      ▼
transport/state or transport/content
      │
      ▼
Rust

composition
      │
      ▼
structural publication interface
      │
      ▼
transport/structural
```

Forbidden:

```text
composition → transport/state
composition → transport/content
api/content → transport/structural
state/content APIs → defineView execution
public declarations → generated ABI records
```

#### 4.2 Rust ownership

A reasonable coarse target is:

```text
crates/iyon-tui/src/
├── retained_state/
│   ├── mod.rs
│   ├── arena.rs
│   ├── property.rs
│   ├── effect.rs
│   ├── dirty.rs
│   ├── damage.rs
│   └── transaction.rs
│
├── content/
│   ├── mod.rs
│   ├── registry.rs
│   ├── capability.rs
│   ├── port.rs
│   ├── connector.rs
│   ├── funnel.rs
│   └── text/
│       ├── source.rs
│       ├── storage.rs
│       ├── projection.rs
│       └── retention.rs
│
├── stream/              # retained during migration; folded/adapted deliberately
├── presentation/
├── scene/
├── application/
└── physical/
```

Do not move every existing stream file merely to match the diagram. Move or rename code only when ownership genuinely changes.

#### 4.3 Current-to-target mapping

| Current subsystem | PERF-13 role |
|---|---|
| `presentation::ir::ViewNode/ViewKind` | structural specification plus immutable initial state values |
| `presentation::layout::LayoutTree` | committed occurrence geometry and parent/dependency metadata |
| `scene::SceneHost` | host integration, retained frame, dirty processing, commit point |
| `presentation::paint` | paint resolution, local repaint, state/content-aware cache keys |
| `physical::Surface` | retained physical frame and rectangle clearing/compositing |
| `stream::StreamingSource/StreamSnapshot/StreamModel` | basis of authoritative text Source and semantic frontier rules |
| `stream::projected/compile/pane` | Funnel projection and Connector-local width/viewport work |
| `history::stream/transfer` | History adapter; not the generic connector scheduler |
| native `TuiHost`/`HostInner` | runtime generation, pending-work epochs, flush/commit ownership |
| generated View ABI | structural plane only |

---

### 5. Identity model

#### 5.1 Identity vocabulary

The implementation must use these terms consistently:

| Identity | Meaning | Mutable target? | Public? |
|---|---|---:|---:|
| `SemanticNodeId` / current TS `NodeId` | immutable semantic View identity | no | no |
| Rust `ViewId` | immutable semantic/cache identity | no | no |
| `NativeRef` | leased native structural object reference | no | no |
| `LayoutNodeId` | occurrence in one committed layout tree | internally, only while that tree is current | no |
| `ViewStateId` | opaque retained mutation attachment | yes | wrapped by `ViewState` |
| `ContentPortId` | opaque structural content-host attachment | yes | wrapped by `ContentPort<T>` |
| `SourceId` | authoritative content producer record | yes | wrapped by source class |
| `ConnectorId` | one Source/Funnel/Port link | yes | wrapped by connector class |
| runtime generation | identifies one live native environment/host generation | validation only | no |

#### 5.2 Why `ViewId` cannot address state

This is invalid:

```text
state.set_background(view_id = 42)
```

because semantic View 42 may be mounted in two places with different rectangles, parents, clips, inherited styles, and content widths.

The stateful form is:

```text
ViewStateId 7
    structurally attached to exactly one committed occurrence
    resolves to the current LayoutNodeId for that occurrence
```

#### 5.3 Handle representation

Do not pass native pointers or JS-safe-integer-packed universal IDs.

Across the content FFI boundary, use explicit lanes:

```c
runtime_slot:       uint32_t
runtime_generation: uint32_t
object_slot:        uint32_t
object_generation:  uint32_t
```

The function name supplies the object kind. A stale generation returns a stable error code. If a generation counter would wrap, retire the slot permanently rather than making an ancient handle valid again.

N-API may wrap the same lanes in opaque class instances. Public TypeScript must never expose the lanes.

#### 5.4 Mount uniqueness

Normative invariants:

- One `ViewState` may be attached to at most one committed occurrence at a time.
- One `ContentPort` may be attached to at most one committed `ContentHost` occurrence at a time.
- A structural node may have at most one `ViewState` and at most one `ContentPort` attachment.
- A `ViewState` or `ContentPort` may be unmounted and later remounted.
- Moving one attachment from an old occurrence to one new occurrence in the same structural transaction is legal.
- Duplicating an attachment in the candidate structural graph is a pre-commit error. The previous graph/frame remains committed.

The duplicate error must report the attachment kind and both candidate occurrence paths. Do not silently choose one.

---

### 6. Structural plane: normative boundary

#### 6.1 Structural means identity/topology/algorithm selection

A field is structural in PERF-13 when changing it changes one of:

```text
node kind / layout algorithm family
parent-child relationship
child order or membership
edge participation identity
structural boundary existence
retained attachment identity
component identity
```

Structural changes continue through the PERF-12 path:

```text
defineView / retained semantic execution
        ↓
changed semantic frontier
        ↓
API-H3 structural publication seam
        ↓
transport/structural
        ↓
Rust retained structural graph
```

#### 6.2 Structural in PERF-13 v1

The following remain structural:

| Concept | Reason |
|---|---|
| `Row`, `Column`, `Grid`, `Container`, `Hanging`, `ClampRows`, `RowViewport`, `ComponentSlot`, `ContentHost` kind | selects a different layout/interaction algorithm |
| parent/child insertion, removal, replacement, reordering | topology |
| axis child `TrackSize` participation (`content`, fixed, flex, content-max) | parent-child edge contract; not a scalar property of only one node |
| grid track count/order/definitions | defines the grid coordinate system and edge relationships |
| grid cell row/column/span/placement | structural relationship to tracks |
| component attachment | retained component identity and host graph |
| `ContentPort` attachment | retained content identity belongs to one structural region |
| container/viewport/clamp/scroll boundary existence | changes clipping/layout/interaction topology |
| clipping-boundary existence | changes ancestor/descendant paint relationship |
| overlay/stack relationship when introduced | topology |

#### 6.3 Mutable geometry state in PERF-13

The first geometry-state tranche includes only scalar/local values whose current semantics can be preserved:

```text
width rule: fit / fill
height rule: fit / fill
padding
min/max width
min/max height
row/column/grid scalar gap
simple alignment owned by the current node
border edge presence under the current one-cell model
```

Caveat: some current builder APIs encode alignment or participation on the parent-child edge. Those remain structural until there is a dedicated retained edge-state design. Do not force an edge property into node state.

#### 6.4 Mutable presentation state in PERF-13

```text
surface background
border color
glyph/style choice for an already-present border
text foreground/background
bold/dim/italic/underline/reverse/strikethrough
StyleRef / sparse StyleSpec
semantic style-state key/value overrides
```

Changing border edge presence is geometry-affecting because it changes the one-cell inset. Changing glyphs or color while the same edges remain is paint-only.

#### 6.5 Interaction/runtime state in PERF-13

Interaction state has distinct owners:

| State | Owner |
|---|---|
| focus/focus-within | Rust host focus manager |
| selected/disabled/active/error/running and application-defined style keys | `ViewState` style-state overrides or control-specific native state |
| scroll offset/follow-end | `ScrollPane`/viewport runtime state |
| connector active/inactive/status | content plane |
| future animation phase | future native clock/state producer; architecture only |

Do not create one universal “interaction map” that lets callers mutate focus, scroll, connector status, and application style keys interchangeably.

#### 6.6 Content policy is not geometry state

For new dynamic content:

```text
wrap mode
text projector/renderer
smoothing/pacing policy
annotation interpretation
```

belong to a Funnel/Connector contract, not to generic node geometry.

Existing static `View.text(...).wrap(...)` semantics may remain in the structural View representation during PERF-13. Do not force migration of all static text merely to claim purity.

#### 6.7 Classification table for current concepts

| Current concept | PERF-13 owner | First release behavior |
|---|---|---|
| View kind | structural | unchanged |
| child list/order | structural | unchanged |
| axis child track | structural edge | unchanged |
| grid tracks/cell spans | structural | unchanged |
| `container()` existence | structural | unchanged |
| clamp/row viewport existence | structural | unchanged |
| padding | geometry state | mutable when a `ViewState` is attached; static fluent value remains initial |
| bounds | geometry state | mutable with native consequence classification |
| fit/fill | geometry state | mutable where represented on the node; edge participation remains structural |
| gap | geometry state | migrate scalar parent gap |
| border edges | geometry + paint | one-cell inset; no arbitrary width |
| border glyph/style/color | presentation | paint-only when edges unchanged |
| surface background | presentation | paint-only |
| text style / StyleRef | presentation | paint-only |
| style-state keys | presentation/interaction | native style resolution |
| focus/focus-within | host interaction | native style resolution |
| `ContentPort` attachment | structural | new `ContentHost` kind |
| connector membership | content retained state | never View topology |
| active connector | content retained state | explicit transactional switch |
| source bytes/revision | content source | never View topology |
| scroll offset | control runtime state | preserved across content switches |

---

### 7. Explicit terminal box model

PERF-13 cannot safely migrate geometry properties without a box model. The following model is normative.

#### 7.1 Every layout occurrence has a box; not every node is automatically a general-purpose painter

Every emitted layout occurrence has:

```text
border_rect   = LayoutNode.rect
content_rect  = rect after border insets and padding
clip_rect     = effective intersection with structural clip boundaries
```

This does **not** mean every semantic View kind gains every decoration method. Public paint capability remains explicit and compatible with current APIs.

#### 7.2 Box order

For each decorated box:

```text
allocated border box
    └── one-cell border edges, if present
          └── padding
                └── content box
```

Intrinsic size is:

```text
content intrinsic size
+ padding insets
+ present border-edge insets
then clamped by min/max bounds
```

#### 7.3 Border semantics

- A present top/bottom/left/right edge consumes exactly one terminal cell on that side.
- `topBottom` consumes only top and bottom rows under the current `BorderSpec` semantics.
- Border color/glyph/style changes are paint-only if edge presence is unchanged.
- Edge presence changes require measurement/placement and old-plus-new damage.
- PERF-13 does not add multi-cell border thickness.
- Existing custom-glyph validation and Unicode rendering semantics must be preserved. Do not silently truncate a wide grapheme to force one-cell behavior.

#### 7.4 Padding

Padding lies inside the border and outside content.

Changing padding:

```text
changes child constraints
changes intrinsic size
may change ancestor size
may move descendants
must damage old and new occupied rectangles
```

#### 7.5 Fill/fit

`fillWidth()` and `fillHeight()` refer to the allocated **border box**.

The content box is the remaining space after border and padding. This makes fill semantics independent of decoration.

#### 7.6 Background

A node surface background fills the box interior, including padding and otherwise-unpainted content cells. Border glyph cells are painted by the border renderer; their foreground/background comes from resolved border/text style semantics.

A child with transparent cells reveals the nearest retained ancestor background, matching the current incremental `clear_rect_with_background` behavior.

#### 7.7 Gap

An axis/grid gap separates adjacent child border boxes. It is not padding on either child.

Changing gap is a mutation of the parent’s geometry state and usually requires:

```text
parent intrinsic remeasure
child placement
old + new child region damage
```

#### 7.8 Clipping

The **existence** of a clipping/viewport boundary is structural. The derived clip rectangle is geometry.

Changing padding, bounds, placement, or viewport size may update the clip rectangle without changing topology.

#### 7.9 Container remains structural

`container()` remains a real structural boundary in PERF-13. It may own box decoration and constraints, but it is not erased simply because some decorations become state fields.

A future simplification may prove that a particular wrapper is semantically redundant. That proof must be differential, not aesthetic.

#### 7.10 `Decorated` migration rule

Do not begin PERF-13 by deleting `Decorated`.

For each current decoration wrapper:

1. characterize its layout, inheritance, clipping, identity, and paint semantics;
2. add the equivalent initial-state representation;
3. render old and new paths differentially;
4. collapse only wrappers with one-to-one semantics;
5. retain a structural box when removing it would merge allocation, inheritance, clipping, or background boundaries.

The goal is:

> **Decoration does not automatically require another structural node.**

The goal is not:

> **No decoration may ever be structural.**

#### 7.11 ContentHost box semantics

`View.content(port)` creates a structural `ContentHost` leaf.

- With no active connector, its content intrinsic size is `0 × 0`; border/padding/bounds still apply.
- With an active connector, intrinsic size comes from that connector’s current projection at the offered width/constraints.
- During a candidate activation, the old active connector’s projection and size remain committed until the candidate is ready.
- A `Container` may wrap a `ContentHost`; a spacer cannot host content.

---

### 8. Public retained-state model

#### 8.1 Decision: one opaque `ViewState`, not raw NodeIds or shareable geometry objects

The public working API is:

```ts
const paneState = tui.viewState();

const pane = View.container(child)
  .state(paneState)
  .padding(1)
  .background(theme.surface);

paneState.setPresentation({
  background: ColorSpec.named("red"),
});

paneState.setGeometry({
  padding: Insets.all(2),
});

paneState.setStyleState("status", "error");
```

`ViewState` is:

- an opaque, Tui-bound, generational handle;
- structurally attached to one occurrence through `.state(...)`;
- logically the mutable state of that occurrence;
- physically allowed to use a host arena record for lifetime, pending values, and mount binding;
- not shareable across two mounted occurrences;
- not a public `NodeId` wrapper;
- not a property-level reactive cell system.

A physical `ViewStateRecord` does not make geometry/presentation a second semantic graph. It is an addressing/lifetime record for one node-owned state bundle.

#### 8.2 Initial values and dynamic overrides

Fluent View modifiers remain ergonomic immutable **initial/base values**:

```ts
View.container(child)
  .padding(1)
  .background(theme.surface)
```

When a `ViewState` is attached, effective state is resolved as:

```text
immutable View base values
    + sparse ViewState overrides
    + host interaction facts (focus/focus-within)
    + Theme resolution
```

Rules:

- A state mutation before mount records an override and is applied on first successful mount.
- A state mutation after mount updates the same retained occurrence.
- Re-publishing a structurally new View with the same `ViewState` may change base values; existing explicit overrides remain authoritative.
- Clearing an override returns that property to the current View base value.
- Inline modifiers are not magically reactive. Reconstructing Views to vary them may still use the structural path. Performance-sensitive dynamic values must use `ViewState`.

Working methods:

```ts
state.setGeometry(patch)
state.clearGeometry(...keys)
state.setPresentation(patch)
state.clearPresentation(...keys)
state.setStyleState(key, value)
state.clearStyleState(key)
state.dispose()
```

Patch calls are typed and atomic at the call level. Do not expose `setProperty(name: string, value: unknown)`.

#### 8.3 Supported geometry patch in PERF-13

```ts
interface GeometryPatch {
  readonly width?: "fit" | "fill";
  readonly height?: "fit" | "fill";
  readonly padding?: InsetsValue;
  readonly minWidth?: number | null;
  readonly maxWidth?: number | null;
  readonly minHeight?: number | null;
  readonly maxHeight?: number | null;
  readonly gap?: number;
  readonly alignment?: AlignmentValue;
  readonly borderEdges?: BorderEdgesValue | null;
}
```

Only expose a field when the current node kind supports it. Invalid node/property combinations are typed or attach-time errors; they are not ignored.

#### 8.4 Supported presentation patch in PERF-13

```ts
interface PresentationPatch {
  readonly foreground?: ColorSpec | null;
  readonly background?: ColorSpec | null;
  readonly borderColor?: ColorSpec | null;
  readonly borderStyle?: BorderStyle | null;
  readonly borderGlyphs?: BorderGlyphs | null;
  readonly textAttributes?: TextAttributesPatch;
  readonly style?: StyleRef | StyleSpec | null;
}
```

#### 8.5 Style-state semantics

Application semantic states remain key/value pairs compatible with API-H3 selectors:

```ts
state.setStyleState("status", "running");
state.setStyleState("severity", "error");
state.clearStyleState("status");
```

The effective node state is:

```text
base immutable StyleStates from View
    overridden/extended by ViewState states
    combined with host facts such as focused/focus-within
```

Theme selectors in PERF-13 are presentation-only. A theme rule must not change padding, bounds, tracks, or other geometry. Geometry-affecting theme rules are rejected/deferred.

#### 8.6 Mount and remount behavior

Lifecycle:

```text
created → unmounted → mounted → unmounted → ... → disposed
```

- Unmounted state retains overrides.
- Remount may target a different compatible View kind.
- Incompatible stored overrides cause candidate-mount validation failure before commit.
- A move from one occurrence to another in one structural transaction retains overrides.
- A duplicate candidate mount fails the structural transaction.

#### 8.7 Disposal

- `dispose()` is idempotent when already disposed.
- Disposing a mounted `ViewState` is an error.
- Disposing an unmounted state invalidates its generation and clears pending overrides.
- Tui shutdown invalidates all Tui-bound `ViewState` handles.
- Finalizers are best-effort cleanup only; correctness must not depend on GC timing.

#### 8.8 No public property bindings in PERF-13

Do not implement:

```ts
View.box().background(bind(state, ...))
```

Property-level dependency compilation is a separate architecture. Callers can subscribe to their own application state and invoke typed `ViewState` mutations.

---

### 9. Rust retained-state representation

#### 9.1 Logical ownership versus physical storage

The state belongs logically to the mounted structural occurrence. A practical physical representation is a host arena:

```rust
struct ViewStateRecord {
    generation: u32,
    lifecycle: StateLifecycle,
    mount: Option<MountedStateBinding>,

    geometry_overrides: GeometryOverrides,
    presentation_overrides: PresentationOverrides,
    style_state_overrides: StyleStateOverrides,

    state_revision: u64,
    geometry_revision: u64,
    presentation_revision: u64,
}

struct MountedStateBinding {
    layout_node: LayoutNodeId,
    structural_generation: u64,
    compatible_kind: StatefulNodeKind,
}
```

The exact fields may differ, but these invariants do not:

- the record is Tui-host-owned;
- one record binds to zero or one committed occurrence;
- the binding is refreshed after successful structural/layout reconciliation;
- an unmounted record may retain overrides;
- cache validation uses relevant revisions, not pointer identity;
- the record cannot be shared as a style object among several nodes.

#### 9.2 Base and effective state

The committed layout/presentation node must expose both:

```rust
struct NodeBaseState {
    geometry: GeometryState,
    presentation: PresentationState,
    style_states: StyleStates,
}

struct NodeEffectiveState {
    geometry: GeometryState,
    presentation: PresentationState,
    style_states: StyleStates,
}
```

`NodeBaseState` is derived from the structural View specification. `NodeEffectiveState` applies retained overrides and host interaction facts.

Do not mutate the immutable structural View object in place.

#### 9.3 Equality before revision

For every patch:

1. validate the value;
2. normalize it to canonical native form;
3. compare it with the currently stored override/effective value;
4. if equal, record a no-op counter and do not advance revisions or dirty flags;
5. otherwise update the pending record, advance the relevant revision, and classify effects.

This avoids the “mutable access implies changed” false-positive problem.

#### 9.4 Property descriptors

Use a finite native property table, not dynamic string dispatch:

```rust
struct PropertyDescriptor {
    id: PropertyId,
    supported_kinds: NodeKindMask,
    baseline_effects: EffectMask,
    validator: fn(&PropertyValue) -> Result<NormalizedPropertyValue, StateError>,
}
```

The TypeScript API lowers typed patches to these stable property IDs through N-API.

#### 9.5 Effect model: bit flags, not an exclusive enum

A mutation can require more than one consequence. Use bit flags:

```rust
bitflags! {
    struct EffectMask: u32 {
        const RESOLVE_STYLE            = 1 << 0;
        const PROJECT_CONTENT          = 1 << 1;
        const MEASURE_SELF             = 1 << 2;
        const MEASURE_ANCESTORS        = 1 << 3;
        const PLACE_SELF               = 1 << 4;
        const PLACE_DESCENDANTS        = 1 << 5;
        const UPDATE_CLIP              = 1 << 6;
        const UPDATE_INTERACTION_GRAPH = 1 << 7;
        const PAINT_SELF               = 1 << 8;
        const PAINT_SUBTREE            = 1 << 9;
        const DAMAGE_OLD_RECT          = 1 << 10;
        const DAMAGE_NEW_RECT          = 1 << 11;
        const STRUCTURE                = 1 << 12;
    }
}
```

`STRUCTURE` is an internal guard. A state-plane public operation must never produce it. If a proposed patch would change topology, reject it and require the structural API.

#### 9.6 Baseline effect table

| Mutation | Baseline effects |
|---|---|
| foreground/background/text attribute | `RESOLVE_STYLE`, `PAINT_SELF`, `DAMAGE_NEW_RECT` |
| border color/glyph/style, same edges | `RESOLVE_STYLE`, `PAINT_SELF`, `DAMAGE_NEW_RECT` |
| style-state key/value | `RESOLVE_STYLE`, `PAINT_SUBTREE` conservatively, `DAMAGE_NEW_RECT` |
| focus/focus-within | `RESOLVE_STYLE`, paint affected component/style subtree |
| padding | `MEASURE_SELF`, `MEASURE_ANCESTORS`, `PLACE_DESCENDANTS`, `UPDATE_CLIP`, old+new damage |
| bounds | `MEASURE_SELF`, `MEASURE_ANCESTORS`, `PLACE_SELF`, `PLACE_DESCENDANTS`, old+new damage |
| width/height fit/fill | measure parent dependency frontier, placement, old+new damage |
| scalar gap | measure the owning parent, place children, old+new damage |
| alignment | placement of affected child/subtree, old+new damage; measure only when current algorithm couples size |
| border edge presence | measurement, placement, clip, paint, old+new damage |
| content revision | `PROJECT_CONTENT`; runtime refines to paint-only or measure/placement after comparing projected metrics |

#### 9.7 Runtime refinement

The baseline mask is conservative. Rust may remove unnecessary work after inspecting committed constraints and old/new metrics.

Example:

```text
text append
    active ContentHost has fixed 80×10 border box
    projection height changes from 100 to 101 rows
    viewport remains 10 rows and follows end

result:
    projection/viewport update
    no ancestor measurement
    paint ContentHost rectangle
```

Contrast:

```text
text append
    ContentHost height = fit
    projected height changes 3 → 4

result:
    measure ContentHost
    propagate height dependency to ancestors
    place affected descendants
    damage old + new regions
```

#### 9.8 Dependency metadata

Targeted layout requires explicit dependency information. Add dependency bits to committed parent-child layout relationships:

```rust
bitflags! {
    struct ChildDependency: u16 {
        const PARENT_USES_CHILD_WIDTH      = 1 << 0;
        const PARENT_USES_CHILD_HEIGHT     = 1 << 1;
        const PARENT_USES_CHILD_BASELINE   = 1 << 2;
        const PARENT_USES_CHILD_INTRINSICS = 1 << 3;
        const CHILD_WIDTH_DEPENDS_ON_PARENT  = 1 << 4;
        const CHILD_HEIGHT_DEPENDS_ON_PARENT = 1 << 5;
    }
}
```

The layout algorithm that consumes the child must record these bits. Do not infer them later from node kinds in a separate duplicated rules table.

If metadata is absent or an algorithm cannot prove a boundary, propagate conservatively to the root. Incorrect extra work is acceptable during staged implementation; incorrect cutoff is not.

#### 9.9 Dirty representation

Use local flags plus generations:

```rust
struct NodeDirty {
    effects: EffectMask,
    width_dirty: bool,
    height_dirty: bool,
    placement_dirty: bool,
    paint_dirty: bool,
    dirty_generation: u64,
}
```

Do not create four unrelated global “structural/geometry/presentation/content revisions” and compare them everywhere. Use:

- host `pending_epoch` / `committed_epoch` for frame scheduling;
- local record revisions for cache validation;
- source revision for content data;
- connector projection revision for attachment-specific derived work;
- structural generation for attachment rebinding.

#### 9.10 Dirty propagation algorithm

Given one changed mounted occurrence:

```text
1. Seed local effect flags from the property descriptor.
2. If intrinsic width/height may change, mark those dimensions dirty.
3. Walk committed parent links upward.
4. For each edge, propagate only dimensions the parent recorded as dependent on.
5. Stop a dimension at a proven dependency boundary.
6. Continue through intrinsic/baseline dependencies even across ordinary boundaries.
7. Mark the smallest placement roots whose descendants may move.
8. Save old rectangles before any candidate layout writes.
9. If metadata is missing or inconsistent, fall back to full-root layout and increment a fallback counter.
```

Pseudo-code:

```rust
fn propagate_measure_dirty(tree: &LayoutTree, start: LayoutNodeId, mut dims: DirtyDims) {
    let mut child = start;
    while !dims.is_empty() {
        mark_measure(child, dims);
        let Some(parent) = tree.parent(child) else { break };
        let dependency = tree.child_dependency(parent, child);
        dims = dependency.propagate(dims);
        child = parent;
    }
}
```

#### 9.11 Measure and placement order

For one candidate frame:

```text
bottom-up:
    project changed content when width/input requires it
    measure dirty leaves/subtrees
    recompute parent intrinsic results along dirty frontier
    stop when output metrics are unchanged and no other effect requires propagation

top-down:
    place the highest dirty placement roots
    update descendant rect/content_rect/clip_rect
    rebind attachment → LayoutNodeId mappings in candidate state
```

If one measure result equals the committed result, upward propagation may stop for that dimension after all local side effects are accounted for.

#### 9.12 Cache keys

Existing measurement/paint caches key heavily on immutable `ViewId`. PERF-13 must add the relevant retained revisions:

```text
measure cache:
    semantic ViewId
    effective geometry revision/fingerprint
    active content connector projection metric revision
    constraints
    component revision as applicable

paint cache:
    semantic ViewId
    effective presentation revision/fingerprint
    active content projection paint revision
    rect/content_rect/clip_rect
    inherited/resolved style context
    theme revision
```

Do not put a universal host revision in every key; that would invalidate the whole cache on any mutation.

#### 9.13 Candidate/committed state

Host state is double-buffered conceptually:

```rust
struct RetainedRuntime {
    committed: CommittedRuntimeState,
    pending: PendingMutationState,
}
```

A frame flush creates a candidate using copy-on-write or scratch structures for only changed records. It must not mutate committed geometry/active connector selection before validation succeeds.

#### 9.14 Frame transaction algorithm

Normative flow:

```text
A. Capture target pending epoch.
B. Drain the latest structural publication(s).
C. Build/reconcile candidate structure and attachment bindings.
D. Validate duplicate/incompatible ViewState and ContentPort attachments.
E. Drain state and connector-control mutations through the captured epoch.
F. Snapshot active/candidate Sources at concrete source revisions.
G. Resolve style/content effects and dirty propagation.
H. Measure and place candidate geometry to convergence.
I. Prepare candidate connector activations and projections.
J. Compute damage from committed old rects and candidate new rects.
K. Paint candidate regions into a candidate surface.
L. Validate frame invariants.
M. Atomically install candidate runtime state, active connectors, surface, and committed epoch.
N. Present/diff through the terminal backend.
```

If steps B–L fail:

- committed structure/state/active connector/surface remain unchanged;
- hard-invalid operations are removed and reported;
- transient work remains pending for retry where meaningful;
- source data already accepted by a Source remains authoritative and will be observed on a later successful frame;
- no blank intermediate connector frame is emitted.

#### 9.15 Error classes during flush

| Error class | Examples | Policy |
|---|---|---|
| caller validation | negative padding, wrong node kind, stale handle | reject operation; report typed error; do not retry |
| structural candidate validation | duplicate port/state attachment | reject candidate root; old root remains |
| capability unsupported | backend lacks required token | connector status becomes unsupported; old active remains |
| geometry blocked | port allocation below minimum | non-fatal blocked status; auto-retry on geometry change |
| transient preparation | temporary allocation/adapter failure | old frame remains; retain retry obligation when safe |
| invariant violation | generation mismatch inside committed graph, impossible parent link | fail loudly; do not silently full-rebuild unless explicitly classified as a recoverable cache miss |

#### 9.16 Convergence

Retain the existing bounded multi-pass convergence model for layout-aware components. State/content dirty processing participates in the same candidate-frame convergence loop.

Do not run an independent content layout loop that can commit geometry separately from the Scene frame.

---

### 10. Damage model

#### 10.1 PERF-13 baseline: rectangle damage

Implement rectangle-level damage first. Cell-range or glyph-range damage is deferred.

```rust
struct DamageRegion {
    rects: Vec<Rect>,
    full: bool,
}
```

#### 10.2 Damage rules

| Change | Damage |
|---|---|
| paint-only property | current border rectangle |
| text/content paint with same geometry | ContentHost viewport/border rectangle |
| move | union of old and new rectangles |
| resize | union of old and new rectangles |
| clip change | old clipped region + new clipped region |
| removed occurrence | old rectangle |
| newly mounted occurrence | new rectangle |
| connector switch, same geometry | port rectangle |
| theme swap | affected styled rectangles; full frame is acceptable for first theme tranche |

#### 10.3 Coalescing

Initial internal constants:

```text
MAX_DAMAGE_RECTS = 64
FULL_DAMAGE_AREA_RATIO = 0.50
```

Algorithm:

1. clamp damage to viewport;
2. discard empty rectangles;
3. merge overlapping or directly touching rectangles;
4. if rectangle count exceeds the cap, use full-frame damage;
5. if union area exceeds 50% of viewport, use full-frame damage;
6. record counters for rectangle count, merged count, damaged cells, and full fallback.

These constants may be tuned by benchmarks without changing semantics.

#### 10.4 Physical surface integration

Reuse the retained `Surface` and its whole-glyph-safe clearing/compositing operations.

For each damage root:

- restore nearest retained ancestor background before compositing transparent child output;
- clear whole wide glyph spans when a changed cell intersects a continuation;
- repaint the smallest safe subtree/region;
- fall back to full paint if style inheritance or overlap prevents a safe local repaint.

#### 10.5 Backend output

PERF-13’s required optimization is reduced Rust layout/paint work. The terminal backend may initially continue diffing a complete final surface.

`PreparedSceneFrame` should carry damage metadata for tests, counters, and future backend optimization. Do not block PERF-13 on partial terminal escape emission.

---

### 11. Theme, focus, and semantic style state

#### 11.1 Theme

Theme replacement becomes native presentation invalidation:

```text
theme revision advances
    → invalidate resolved-style and paint cache entries
    → repaint affected nodes/full frame
    → no semantic View reconstruction
    → no layout unless a future theme system explicitly supports geometry
```

PERF-13 themes remain presentation-only.

#### 11.2 Focus

Focus/focus-within already live in the Rust host/mount graph. PERF-13 must stop requiring a semantic View replacement merely to reflect focus-dependent appearance.

Focus transition:

```text
old focused path + new focused path
    → resolve style context
    → paint affected component/style subtrees
    → no layout
```

#### 11.3 Application style states

`ViewState` key/value states are merged with immutable base states and host facts. A state change must invalidate descendants only when selectors/inheritance can observe it.

For the first implementation, conservative subtree paint is acceptable. Add selector-dependency indexing only if benchmarks show it is necessary.

#### 11.4 Animation

PERF-13 does not implement a general animation API.

It must, however, leave a native mutation seam that a future Rust-owned clock can call without TypeScript per-frame execution. Existing `ViewSlot` animation remains supported and is not rewritten unless required for integration.

---

### 12. Content-plane entity model

#### 12.1 Normative graph

```text
Source
    │ authoritative width-independent semantic data
    ▼
Funnel
    │ immutable typed transformation/delivery contract
    ▼
Connector
    │ one retained attachment and its derived state
    ▼
ContentPort
    │ one structurally mounted receiving region
    ▼
ContentHost layout occurrence
```

#### 12.2 Source

A `Source` owns authoritative semantic content independent of where it is displayed.

For text it owns:

```text
UTF-8 content/storage
absolute logical byte coordinate range
revision
retention policy/state
seal state where applicable
width-independent annotations/provenance
```

A Source:

- does not know View topology;
- does not own port width;
- does not own scroll state;
- may be shared by several Connectors;
- may outlive a Tui host;
- advances while all Connectors are inactive/cold;
- is retained in Rust, not reconstructed from a JS String on every frame.

#### 12.3 Funnel

A Funnel is an immutable typed value/configuration, not a separately mutable runtime handle in PERF-13.

It owns:

```text
input Source family
output content family
semantic projector/renderer selection
wrap/align/presentation policy appropriate to that family
backend requirements
minimum useful geometry requirements
configuration fingerprint
```

Examples:

```text
PlainUtf8BlockFunnel
PlainUtf8StreamFunnel
existing Markdown stream projector adapter, if needed for Iyon migration
```

A Funnel does not own:

```text
active/inactive state
port geometry
scroll position
width-specific row cache
source bytes
scheduler priority
```

The implementation may intern identical Funnel specs, but that is an optimization, not semantic identity.

#### 12.4 Connector

A Connector is a Tui-bound retained identity linking exactly:

```text
one Source
one Funnel spec
one ContentPort
```

It owns attachment-local state:

```text
connector lifecycle/status
activation request/status
width-specific projection and wrap cache when active
last committed source revision
projection revision
measurement summary
backend-placement identity in future
```

In PERF-13, an inactive cold Connector owns no projected rows, no inactive queue, and no layout-ready shadow state.

#### 12.5 ContentPort

A ContentPort is a Tui-bound retained identity structurally attached to one `ContentHost` occurrence.

It owns:

```text
semantic accepted content family
current mounted geometry/viewport
connector membership
0..1 active connector
current committed active projection
activation candidate during a frame transaction
```

A port does not own Source data and does not perform policy arbitration.

#### 12.6 Source sharing

The same Source may feed several Connectors:

```text
                      TextStreamSource
                      /              \
                     /                \
          Connector A @ width 80   Connector B @ width 32
                   │                     │
                 Port X                Port Y
```

Each Connector has an independent width-specific projection cache. The Source stores width-independent semantic content once.

#### 12.7 Port multiplicity

Invariant:

```text
ContentPort.connectors: 0..N
ContentPort.active:     0..1
```

Connector membership and active selection are retained content-plane state. They do not alter the structural View DAG.

#### 12.8 Public working API

```ts
const source = TextStreamSource.create({
  retention: { maxBytes: 8 * 1024 * 1024, overflow: "drop-oldest" },
});

const port = tui.contentPort(TextContent);

const connector = port.connect(
  source.funnel({
    renderer: "plain",
    wrap: "word",
  }),
);

const pane = View.content(port)
  .fillWidth()
  .fillHeight();

connector.activate();
source.append("hello\n");
```

Working public concepts:

```ts
ContentPort<TContent>
ContentConnector<TContent>
TextBlockSource
TextStreamSource
Funnel<TContent>
TextContent
```

Do not expose a universal `content(anything)` API or untyped option bag.

#### 12.9 Why `View.content(port)` is explicit

Only the structural `ContentHost` kind may host retained content in PERF-13.

This gives deterministic errors and prevents accidental APIs such as:

```ts
View.spacer(2).content(video)
```

A caller may wrap a ContentHost in Container/Grid/Row/Column structure as needed.

---

### 13. Capability model

Capability checks happen at three distinct levels.

#### 13.1 Semantic family compatibility

Examples:

```text
Text funnel → TextContent port       valid
Graphics funnel → TextContent port   invalid
```

Represent this twice:

- TypeScript generic compatibility for ordinary callers;
- a compact Rust `ContentFamilyId` runtime check for ABI safety and dynamic paths.

Mismatch is a synchronous `CONTENT_FAMILY_MISMATCH` error at `connect()` time.

#### 13.2 Backend capability

A Funnel may require backend tokens:

```text
plain terminal text  → no special token
future Kitty image   → kitty.graphics
future Sixel         → sixel
```

Requirements are a small sorted token set or typed enum family, not a mutable capability god-object.

Backend checks occur when:

- the port is mounted into a host with a known backend;
- a connector is activated;
- backend capabilities change, if that becomes possible.

An unsupported Connector remains connected but has status `unsupported-backend`. Activation fails without replacing the current active Connector.

#### 13.3 Geometry readiness

A Funnel may declare minimum useful geometry:

```rust
struct GeometryRequirement {
    min_columns: u16,
    min_rows: u16,
}
```

Geometry is known only after layout.

If a candidate Connector cannot operate in the current allocation:

- status becomes `blocked-geometry`;
- the previous active Connector remains visible;
- the activation request remains pending;
- the runtime retries automatically when port geometry or Funnel/Source requirements change.

Geometry blocking is a status, not a thrown frame-fatal error.

#### 13.4 Allocation negotiation is future-specific

Text content copies bytes into Rust-owned storage and does not require GStreamer-like buffer-pool negotiation.

The architecture leaves room for a future graphics Funnel to negotiate surfaces/placement allocation separately from semantic compatibility. Do not implement a generic allocation protocol in PERF-13.

#### 13.5 Errors

Examples:

```text
Cannot connect Graphics funnel:
port accepts TextContent.

Cannot activate connector 17:
backend does not advertise kitty.graphics.

Connector 23 is blocked:
requires at least 2 rows; port currently allocates 1.
```

Every error/status includes:

```text
stable code
connector/port identity for diagnostics
required capability
actual capability/allocation
operation that failed
```

---

### 14. Connector lifecycle and cold semantics

#### 14.1 Connector state machine

```text
created/inactive
      │ activate()
      ▼
preparing
  ├── compatible + prepared ──► active
  ├── insufficient geometry ──► blocked-geometry
  ├── backend missing ────────► unsupported-backend
  └── hard projection error ──► failed

active
  ├── another connector commits ─► inactive
  ├── port.deactivate() ─────────► inactive
  ├── disconnect/dispose ────────► disposed
  └── source changes ────────────► active + dirty
```

`blocked-geometry` and `unsupported-backend` are inactive states. They retain the activation request where automatic retry is meaningful.

#### 14.2 Exact cold definition

An inactive cold Connector:

- performs no projection;
- performs no wrapping/line compilation;
- performs no measurement or layout;
- paints nothing;
- retains no inactive delivery queue;
- retains no inactive projected rows/surface;
- does not receive per-update callbacks for every Source revision beyond cheap host/source dirty bookkeeping;
- retains only identity, immutable Funnel config, Source/Port leases, lifecycle/status, and minimal diagnostics.

This is distinct from Source retention.

#### 14.3 Source advancement while cold

Sources always accept semantic updates while Connectors are cold.

Example:

```text
Source revision 1: "hello"
Connector inactive
Source revision 2: "hello world"
activate Connector
→ candidate synchronizes from revision 2
```

No connector buffering occurred.

If a Source intentionally retains only a tail and old data was truncated, activation sees the retained tail. Cold does not recreate discarded Source history.

#### 14.4 No automatic first activation

Creating the first Connector does not auto-activate it.

```ts
const connector = port.connect(funnel);
// port is still empty
connector.activate();
```

A future/convenience factory may create+connect+activate in one explicit helper, but the core state machine never relies on “first wins.”

#### 14.5 Ports may be empty

A ContentPort may have no active Connector. It renders an empty content box according to §7.11.

Working API:

```ts
port.deactivate();
```

#### 14.6 Transactional activation

Activation must use prepare/commit semantics:

```text
1. Record activation request and sequence.
2. Keep current active Connector committed.
3. Validate semantic family and backend requirements.
4. Snapshot candidate Source at a concrete revision.
5. Project/compile for current port geometry into candidate storage.
6. Validate geometry and projection invariants.
7. Compute candidate measurement/layout and damage.
8. Commit active Connector + projection + frame together.
9. Only then release old active projection state.
```

There is no blank or partially projected intermediate frame.

#### 14.7 Source change during preparation

A Source snapshot is immutable for the duration of candidate preparation.

After preparation:

- if the Source revision is unchanged, commit normally;
- if it advanced, the runtime may commit the captured internally consistent snapshot and immediately leave the active Connector dirty for the next frame;
- do not restart indefinitely under a continuously writing Source.

For the current single-host-lock path, concurrent advancement may be rare, but the invariant must still be correct for future worker/native producers.

#### 14.8 Activation failure

If activation fails:

```text
old active Connector remains active and visible
candidate status records the failure
port active identity is unchanged
structural DAG is unchanged
no partial candidate projection is retained for cold mode
```

If there was no old active Connector, the port remains empty.

#### 14.9 Switching order

Multiple activation requests in one pending batch are last-request-wins for the same port, provided every superseded request has not already committed.

```text
A.activate()
B.activate()
C.activate()
flush
→ prepare/commit C only
```

Record coalescing counters for superseded requests.

A call to `tui.flush()` is a barrier. Requests after that barrier belong to a later frame.

#### 14.10 Disconnect and dispose

- Disposing an inactive Connector detaches it at the next commit and releases Source/Port leases.
- Disposing the active Connector makes the port empty at commit unless another activation in the same batch succeeds.
- A replacement activation and active-Connector disposal in the same batch commit atomically with no empty intermediate frame.
- `dispose()` is idempotent after completion.

#### 14.11 No scheduler in PERF-13

Do not implement:

```text
priority
preemption
blocking ownership
yield-on-seal
activate-on-ready
automatic fallback
policy-based connector selection
```

The application calls `activate()`.

---

### 15. Text content families

Text is the proving family. PERF-13 ships two Source semantics that both produce the same `TextContent` output family through typed Funnels.

#### 15.1 UTF-8 Block Source

Snapshot-like content.

Operations:

```text
replace(bytes/text, optional metadata)
clear()
snapshot/query revision
```

No append operation is exposed. A Block replacement is one atomic Source-state swap.

#### 15.2 UTF-8 Stream Source

Ordered evolving content.

Operations:

```text
append(text, optional annotations)
replace(text, optional annotations)
clear()
seal()
snapshot/query revision
```

`append` and `replace` are semantically distinct and must never be guessed from payload shape.

#### 15.3 Exact append semantics

`append(x)`:

- validates UTF-8 at the native boundary;
- appends bytes after the current logical end;
- assigns absolute byte coordinates;
- appends annotations relative to the appended payload after validation/lowering;
- advances Source revision once for the logical call;
- applies retention atomically;
- preserves ordering among append calls.

#### 15.4 Exact replace semantics

`replace(x)`:

- constructs a fresh candidate text store and metadata off to the side;
- resets the retained logical range according to the Source’s coordinate policy;
- swaps it atomically on success;
- invalidates all semantic projection from the new source base;
- advances revision once;
- is not implemented as `clear(); append(x)` because that would expose an intermediate empty revision and incorrect coalescing semantics.

The coordinate policy for PERF-13 text replacement is:

```text
source_base = 0
source_end  = encoded byte length
```

This matches a snapshot replacement. Existing rolling-stream adapters that require monotonic external coordinates must use the existing `StreamSnapshot` adapter path until they are deliberately migrated.

#### 15.5 Clear

`clear()` is equivalent to an atomic replace with empty text for content semantics, but has its own ABI/control opcode so no empty byte pointer is required.

#### 15.6 Seal

`seal()` is a one-way semantic transition:

```text
open → sealed
```

After seal:

- append/replace/clear fail with `SOURCE_SEALED`;
- Source content remains readable/projectable;
- Connector activation remains legal;
- History may use the sealed frontier for freezing/promotion;
- a second `seal()` returns `SOURCE_ALREADY_SEALED` to preserve strict bug detection and current behavior.

Seal does not automatically activate a Connector or freeze History.

#### 15.7 Existing `TextStream.update`

The existing public `TextStream.update(text)` becomes a compatibility adapter to new Stream `replace(text)` semantics. Do not retain a separate native update implementation.

#### 15.8 Plain and Markdown Funnels

Mandatory first Funnels:

```text
PlainUtf8BlockFunnel
PlainUtf8StreamFunnel
```

The existing Markdown projector may be adapted into a Stream Funnel if Iyon migration requires it. Do not rewrite Markdown as a new document engine in PERF-13.

#### 15.9 Rich text and annotations

PERF-13 must preserve current stream annotation capability needed by Iyon, but it need not design a universal rich-document model.

Internal rule:

```text
text bytes use absolute UTF-8 byte coordinates
annotation ranges are validated against UTF-8 boundaries
width-independent annotations live with Source/Funnel semantic projection
width-specific painted runs live in Connector projection
```

Interaction annotations and arbitrary editable rich-text operations are future work.

---

### 16. Text Source retention

#### 16.1 Retention is Source policy, not Connector standby policy

```text
Source retention:
    which semantic bytes remain authoritative?

Connector cold policy:
    does an inactive attachment perform/retain derived delivery work?
```

They are independent.

#### 16.2 Default

Default retention is unbounded.

Rationale: silent truncation is a semantic data-loss policy and must never be the framework default.

#### 16.3 Configurable policy

```ts
interface TextRetentionPolicy {
  readonly maxBytes?: number;
  readonly maxLines?: number;
  readonly overflow: "drop-oldest" | "error";
}
```

At least one limit must be present when a policy is supplied. Limits must be positive safe integers and are converted to bounded native sizes.

If both limits are present, the Source must satisfy both after each mutation.

#### 16.4 `overflow: "error"`

The entire append/replace operation is rejected atomically with `SOURCE_RETENTION_OVERFLOW`. Source bytes, revision, annotations, and connector-visible content remain unchanged.

#### 16.5 `overflow: "drop-oldest"`

The Source advances `source_base` and removes the oldest retained content.

Rules:

- `maxLines` removes complete oldest logical lines.
- `maxBytes` first removes complete oldest lines where possible.
- If a single remaining logical line exceeds `maxBytes`, retain a UTF-8-safe suffix and mark the retained head as partial.
- Never split a UTF-8 code point.
- Annotation ranges before the new base are discarded; crossing ranges are clipped only when their annotation type permits clipping, otherwise discarded according to the current annotation contract.
- Source revision advances once for the caller operation, not once per removed chunk.

#### 16.6 Retention and coordinates

Use absolute logical byte offsets internally. Advancing `source_base` must not renumber retained bytes.

This preserves existing `StreamSnapshot`/frontier reasoning and lets Connectors recognize that their previous projection prefix is no longer available.

#### 16.7 Retention observability

Expose read-only source statistics through testing/diagnostics, not the ordinary hot path:

```text
revision
source_base
source_end
retained_bytes
retained_lines
chunk_count
sealed
head_partial
```

---

### 17. Native text storage and projection

#### 17.1 Required complexity

The first implementation must provide:

```text
append                  O(new bytes + new line breaks), amortized
replace                 O(replacement bytes)
head truncation         O(chunks/lines removed), not O(total retained bytes)
line lookup             indexed
snapshot                no full UTF-8 copy merely to inspect revision/range
width-specific compile  Connector-local and incremental where existing model permits
```

Repeatedly rebuilding one giant Rust `String` is not acceptable.

#### 17.2 Storage shape

Recommended initial representation:

```rust
struct TextStore {
    source_base: u64,
    source_end: u64,
    chunks: VecDeque<TextChunk>,
    line_starts: VecDeque<u64>,
    annotations: AnnotationStore,
    revision: u64,
    sealed: bool,
    head_partial: bool,
}

struct TextChunk {
    start: u64,
    bytes: Arc<[u8]>,
}
```

Requirements:

- chunks are immutable after insertion;
- append may coalesce very small adjacent writes into a bounded tail builder before sealing a chunk;
- chunk thresholds are internal benchmark-tunable constants;
- snapshots share immutable chunks instead of copying all bytes;
- line starts are absolute logical byte offsets;
- every committed chunk is valid UTF-8.

#### 17.3 Append algorithm

```text
1. Validate handle/runtime/source lifecycle.
2. Validate byte slice as UTF-8.
3. Validate/convert annotation sidecar against the encoded byte length.
4. Build a candidate tail chunk or merge into the bounded tail builder.
5. Scan only new bytes for line boundaries.
6. Compute candidate source_end and retention removals.
7. If overflow=error and a limit would be exceeded, reject with no mutation.
8. Apply candidate chunk/index/annotation changes under the Source lock.
9. Advance revision once.
10. Mark subscribed active Connector/host pending epochs.
11. Return before any projection/layout/paint.
```

#### 17.4 Replace algorithm

```text
1. Validate bytes/annotations and retention in a fresh TextStore builder.
2. Build chunks and line index off to the side.
3. Set base=0 and end=byte length.
4. Swap store atomically.
5. Advance revision once relative to the Source record’s prior revision.
6. Mark active subscribers dirty.
```

Do not mutate the old store incrementally and leave it partially replaced on allocation failure.

#### 17.5 Snapshot representation

A Source snapshot must be immutable and cheap to clone:

```rust
struct TextSourceSnapshot {
    revision: u64,
    source_base: u64,
    source_end: u64,
    stable_through: u64,
    chunks: Arc<ChunkSequence>,
    line_index: Arc<LineIndex>,
    annotations: Arc<AnnotationSnapshot>,
    sealed: bool,
    head_partial: bool,
}
```

Reuse/reshape the existing `StreamSnapshot` and `StreamModel` concepts instead of maintaining two unrelated correctness models.

#### 17.6 Source/Funnel/Connector split for text

```text
Source
    raw authoritative UTF-8 + source annotations + revision

Funnel
    width-independent semantic projection policy
    e.g. plain or existing Markdown projector

Connector
    projection instance at one port width
    row index / wrap cache / viewport-facing compiled result
```

Current `ProjectedText`-style width-independent semantics may remain in the Funnel stage. Width-specific row compilation belongs to the Connector.

#### 17.7 Connector projection cache

Key:

```rust
struct TextProjectionKey {
    source_revision: u64,
    source_base: u64,
    width: u16,
    funnel_fingerprint: u64,
    wrap_mode: WrapMode,
    alignment: TextAlign,
}
```

The value stores:

```text
compiled rows or row index
intrinsic width/height summary
stable semantic/visual frontier
paint runs/styles
projection revision
```

An inactive cold Connector must drop this value.

#### 17.8 Incremental projection

Reuse current semantic/visual restart-frontier logic:

- append-only changes may restart from the prior unstable frontier;
- replacement restarts from the new Source base;
- head truncation invalidates data before the new base and repairs viewport anchors;
- width change invalidates width-specific rows but not Source data;
- a shared Source at two widths compiles independently.

#### 17.9 Unicode invariants

- FFI accepts bytes and validates UTF-8 in Rust even when bytes came from `TextEncoder`.
- Logical coordinates are UTF-8 byte offsets.
- Exact range operations require code-point boundary validation.
- Grapheme-safe painting remains the physical layer’s responsibility.
- Retention never leaves an invalid UTF-8 prefix/suffix.
- Wide-glyph continuation rules in `Surface` remain authoritative.

#### 17.10 Line semantics

Preserve current newline/wrapping semantics. Do not silently normalize CRLF, strip carriage returns, or reinterpret Unicode line separators during the storage refactor unless the current semantic layer already does so.

Add differential tests for:

```text
LF
CRLF
bare CR
combining marks
emoji ZWJ sequences
wide CJK glyphs
invalid UTF-8 ABI input
head truncation adjacent to multibyte code points
```

---

### 18. Content measurement contract

#### 18.1 Measurement inputs

A Text Connector receives:

```text
Source snapshot
Funnel config
available content width / constraints
port viewport state
```

It produces:

```text
intrinsic content size
compiled rows / projection
paint data
projection revision
```

#### 18.2 Ownership

```text
Source:
    width-independent data/index/revision

Funnel:
    immutable transform policy

Connector:
    width-specific projection/cache/measurement

Port/ContentHost:
    allocated geometry and clip
```

Width-specific caches must not live solely on Source because one Source can be attached at different widths.

#### 18.3 Fixed versus fit allocation

For a fixed/fill ContentHost:

- Source revision may change projection and paint while allocated size stays fixed.
- Ancestor measurement is skipped when candidate intrinsic changes cannot escape the fixed constraints.

For a fit ContentHost:

- changed projection metrics propagate through recorded parent dependencies.

Rust decides this after projection; TypeScript does not submit `invalidateLayout` hints.

#### 18.4 Empty/no-active measurement

A port with no active Connector reports zero intrinsic content size. Bounds, padding, border, and parent fill rules still apply.

#### 18.5 Candidate activation measurement

Activation preparation measures the candidate off to the side. The committed layout continues using old active metrics until candidate commit.

If candidate metrics change ancestor geometry, the connector switch and the resulting layout commit atomically.

---

### 19. Transport architecture

#### 19.1 Four distinct contracts

```text
STRUCTURAL CONTROL/DATA
    existing retained View ABI and structural publication

STATE CONTROL
    N-API lifecycle + typed property patches

CONTENT CONTROL
    N-API Source/Port/Connector lifecycle, config, status, activation

CONTENT DATA
    mandatory direct FFI UTF-8 payload submission
```

Do not extend the structural View schema with large content payload records.

#### 19.2 N-API responsibilities

N-API owns:

```text
create/dispose ViewState
set geometry/presentation/style-state patches
create/dispose Source records and return internal handle lanes
create/dispose ContentPort
create/dispose Connector
connect/detach
activate/deactivate
query connector status
query backend capabilities
configure retention/funnel specs
flush host
map native errors to TuiError
```

#### 19.3 Content FFI responsibilities

All UTF-8 payload bytes use direct FFI, including tiny updates:

```text
text block replace
text block clear
text stream append
text stream replace
text stream clear
text stream seal (no bytes but belongs to the same data sequencing API)
annotation sidecar submission when migrated
```

This avoids a permanent “small N-API / large FFI” semantic split.

#### 19.4 Example C ABI

Illustrative v1 signatures:

```c
int32_t iyon_content_text_stream_append_utf8_v1(
    uint32_t runtime_slot,
    uint32_t runtime_generation,
    uint32_t source_slot,
    uint32_t source_generation,
    const uint8_t *bytes,
    size_t len,
    uint32_t sequence_low,
    uint32_t sequence_high
);

int32_t iyon_content_text_stream_replace_utf8_v1(
    uint32_t runtime_slot,
    uint32_t runtime_generation,
    uint32_t source_slot,
    uint32_t source_generation,
    const uint8_t *bytes,
    size_t len,
    uint32_t sequence_low,
    uint32_t sequence_high
);

int32_t iyon_content_text_stream_clear_v1(...);
int32_t iyon_content_text_stream_seal_v1(...);
int32_t iyon_content_text_block_replace_utf8_v1(...);
int32_t iyon_content_text_block_clear_v1(...);
```

The exact generated names may differ, but the ABI must include:

```text
ABI version
runtime generation
object slot + generation
ordered sequence or equivalent linearization token
stable integer status
pointer + byte length for payload calls
```

#### 19.5 ABI metadata probe

At startup, `transport/content/ffi.ts` must verify:

```text
abi_name = iyon_tui_content
abi_version = 1
semantic_version = 1
schema/header hash
native runtime generation compatibility
required symbol count
pointer/usize assumptions for platform
```

A mismatch is a startup error with no silent payload fallback.

#### 19.6 One adapter

Only `transport/content/ffi.ts` may import `bun:ffi` or construct raw pointers.

Public/API/runtime modules call a typed `ContentDataTransport` interface. Tests may inject an oracle implementation, but production has one implementation after the final tranche.

#### 19.7 String encoding

Use one `TextEncoder` per module/runtime.

Recommended policy:

- maintain a reusable scratch `Uint8Array` for small/medium strings;
- grow it to a safe upper bound for one `encodeInto` call, up to an internal cap;
- for large strings, use one exact `TextEncoder.encode` allocation;
- perform one FFI payload call per public logical append/replace;
- pass the TypedArray pointer only for the duration of the synchronous call.

Do not split one logical append into several visible Source revisions merely to fit scratch storage.

#### 19.8 Memory ownership

The FFI function must synchronously:

1. validate the pointer/length under the supported runtime contract;
2. validate/copy bytes into Rust-owned candidate storage;
3. complete or reject the Source mutation;
4. return before JS may reuse/free the TypedArray.

Rust must never retain the JS pointer.

#### 19.9 Annotation sidecar

When current Iyon annotations migrate, use a compact typed sidecar ABI, for example parallel `Uint32Array` lanes plus a style/config table referenced through N-API-created IDs. Do not send arbitrary JS objects through the byte FFI path.

#### 19.10 FFI support gate

Bun FFI is a platform/runtime risk and must be treated explicitly:

- support every staged-native platform already claimed by the project;
- add startup probes and CI smoke tests for each;
- add long-running append/replace/GC/teardown soak tests;
- never retain raw pointers;
- keep an N-API mirror only as a development/differential oracle behind a non-production flag;
- delete or make that oracle test-only in the final tranche;
- fail clearly when production FFI is unavailable rather than silently changing architecture.

PERF-13 does not add new platform targets; it preserves the existing support matrix.

#### 19.11 Error status codes

The data ABI returns stable integer statuses such as:

```text
0  OK
1  INVALID_RUNTIME
2  STALE_HANDLE
3  WRONG_HANDLE_KIND
4  DISPOSED
5  SOURCE_SEALED
6  INVALID_UTF8
7  RETENTION_OVERFLOW
8  INVALID_SEQUENCE
9  OUT_OF_MEMORY
10 INTERNAL_INVARIANT
```

Map these to typed TypeScript `TuiError` codes outside the FFI hot function. The FFI function must not allocate error strings on success paths.

---

### 20. Scheduling, coalescing, and ordering

#### 20.1 No render per mutation

State/content methods enqueue/update native semantic state and return. They do not project/layout/paint synchronously unless the caller explicitly reaches a flush/read barrier.

#### 20.2 Native pending epoch is authoritative

Each Tui host owns:

```rust
pending_epoch: u64
committed_epoch: u64
```

Every host-affecting mutation atomically advances `pending_epoch` or associates work with the current pending epoch.

The TypeScript microtask scheduler is only a wake hint. Correctness must not depend on a JS boolean saying “scheduled.” If a wake is lost, native `pending_epoch != committed_epoch` still proves work exists.

#### 20.3 Source subscriptions

A Source record tracks weak subscriptions from Connectors to their Tui hosts. A successful Source mutation marks only hosts with affected active or activation-pending Connectors dirty.

Inactive cold Connectors do not project, but an activation-pending Connector’s host must wake so it can prepare the latest revision.

#### 20.4 TypeScript host scheduler

One scheduler per Tui wrapper:

```text
markWakeHint()
    if no microtask currently queued:
        queue one microtask

microtask:
    call native flush-through-current-epoch once
    if native still reports pending work:
        queue another microtask
```

Do not spin synchronously under continuous producers. Process one captured epoch, then yield and reschedule.

#### 20.5 Read-your-writes barriers

The following force a flush before returning observable frame state:

```text
tui.flush()
screenRows()
nativeHistoryRows()
styleAt()
cellXOfText()
headless/testing frame snapshot
```

Input dispatch, resize, backend poll, and event-loop sleep must also process pending work before observing or waiting.

#### 20.6 Structural publication ordering

For one host flush:

```text
1. latest structural publication through the captured epoch
2. candidate attachment reconciliation
3. retained state patches
4. connector activation/deactivation/disposal control
5. latest Source snapshots/revisions
6. one projection/layout/paint/commit
```

This permits:

```ts
const state = tui.viewState();
state.setPresentation({ background: red }); // unmounted override
await publishRoot(View.container(...).state(state));
tui.flush();
```

The first committed mount already uses the override.

#### 20.7 State coalescing

Within one uncommitted epoch range:

- last write wins per `(ViewStateId, PropertyId)`;
- setting a value and then clearing the override resolves to the final base value;
- equal final value is a no-op;
- style-state writes coalesce per key;
- different properties remain one atomic candidate state.

#### 20.8 Content ordering

Source operations are linearized in call/sequence order.

Examples:

```text
append A; append B
    → AB

append A; replace X; append B
    → XB

replace X; clear; append B
    → B
```

Adjacent appends may be physically coalesced into one chunk, but Source revision/ordering semantics must remain testable.

#### 20.9 Activation is not a source snapshot barrier

`connector.activate()` requests that the Connector synchronize to the latest Source revision at the next commit.

```text
activate(); append("x"); flush();
```

The activated Connector may show `x` in that first committed frame.

To force a boundary:

```text
activate(); flush(); append("x");
```

#### 20.10 Public transaction API

PERF-13 does not require a general public `tui.transaction()` or property-binding compiler.

Automatic epoch coalescing plus explicit `tui.flush()` provides the necessary semantics. A small `tui.batch()` convenience may be proposed later, but no tranche may depend on it.

#### 20.11 Failure and retry

- Invalid caller operations are removed and surfaced once.
- A failed candidate frame does not advance `committed_epoch` through the failed work.
- Transient retryable work remains discoverable from pending records/epochs.
- There is no separate “dirty but not scheduled” state whose boolean can become stale.

---
