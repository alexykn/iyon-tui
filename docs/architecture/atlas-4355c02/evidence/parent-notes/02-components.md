# Parent reading notes:02 components
Fully read1–1577 in chunks1–500,501–1040,1041–1577. Static scout evidence, source cross-check still pending.

## Integration
- Registry sole owner Box<dyn ErasedComponent>, Component 'static NOT Send/Sync. Typed Copy nonowning handle PhantomData<fn()->C>, globally monotonic nonzero checked AtomicU64 relaxed; no generations/no reuse, raw unknown ID deferred MissingComponent. Public item declarations not external public Rust API.
- Component view+capabilities immutable declaration outputs; capability implementation owned interaction/command.rs, component/capability.rs facade. Snapshot single-entry cache View+revision+caps RefCell; successful mutable access invalidates/increments even Ignored/false/no semantic change; panic before callback completion can escape invalidation (not raised by scout—check relevant unwind boundary).
- Capabilities focusable/modal, focus/paste/layout/contentextent singleton replacement; ordered key command append; singleton tick. Erased Arc callbacks Any downcasts invariant panic. Layout/contentextent crateprivate.
- Registry object lifetime != scene mount graph reachability. Native retirement delayed until graph unmounted; Rust AppCx remove immediate caller responsibility. Key reconciliation distinction: SceneHost successful preparation vs backend receipt logical visible commit—scout calls both committed, investigate exact rollback before final prose.
- MountGraph HashMap entries+ordered root/children vectors, DFS iterator; revisions separate. Mount transitions membership only, unmount reverseDFS children first/mount DFS parents first; reorder/reparent/revision no transitions. same_topology compares ordered id,parent ignoring revisions.
- Resolver scans only identity-flagged branches, missing/duplicate/cycle explicit; snapshot view recursively scanned into overlay/caps/graph. Slot shell invisible physical metadata.
- Local replace_subtree keeps owner/removes old descendants; host verifies duplicates outside subtree. Graph lowlevel trusts parent/cycle and external collision invariants; parent walker no cycle guard. Internal trust boundary, not automatically bug.
- TickScheduler mounted HashSet/order vector/registrations, scans O(mounted) deadline and due, executes due mount order, resets now+interval no catchup phase. Dynamic interval changes reset. Tick emits outputs independently dirty bool; mutable callback advances revision even false. SceneHost invalidates changed components.
- Native HostViewSlot lives application/host not empty component/slot.rs. Actual component slot lives presentation IR. Outer MountedViewSlot16ms even static/oneframe, inner caller animation interval; empty frames rejected, zero inner interval apparently allowed; clock/frames/pending cycle boundary replacement all native.
- Perf view/capability/resolver counters; resolver counter increments before pruning. No mount/tick/retirement diagnostics. Existing component557 testlines/static evidence only.

## Reconciliation/issues
1. Empty slot.rs residue versus intended oracle ownership, not functional defect by itself.
2. Zero animation interval and always16ms static slot registration: check TS validation and API-H/PERF13 intent; likely overhead/contract distinction, not prohibited per-tick JS.
3. Non-Send callback/thread-affinity + unsafe host thread marker grouped with01.
4. Mutable access revision despite false tick can generate internal invalidation and capability refresh—need actual SceneHost scheduling to quantify idle overhead.
5. Raw component ID validation/native cross-host authority relevant17/22.
6. Successful preparation mount graph versus receipt-visible authority/retirement wording could overclaim transactionality; inspect03/40 and host source.
7. Item pub/private is consistent implementation not contradiction/defect despite report wording.
8. Graph construction trust only a maintenance hazard unless reachable invalid producer found.
Related03,12,13,16,17,38,39,40.
