# Parent reading notes: 01 application
Full report read lines 1–1854 (chunks 1–470,471–950,951–1410,1411–1854). Claims below are scout evidence, not yet independent source confirmation.

## Integration facts
- Distinguish internal generic App/RunningApp callback/action/timer kernel from production TuiHost/HostInner backend/receipt/epoch runtime. application module crate-private, generic driver tests-only. Do not document a public production Rust App loop.
- App init FnOnce(&mut AppCx)->Result<State,E>; update FnMut(&mut State,Action,&mut AppCx)->Result<(),E>; view Fn(&State)->View. App creates ingress, not terminal/task. Theme Arc COW.
- Ingress bounded1024 with recoverable Full/Closed, send_async capacity waiting. Internal VecDeque unbounded, collect/update128 budgets; timers vector O(n), checked IDs/deadlines panic on exhaustion. ReadyStatus dirty/exiting/more_ready.
- advance_ready: preexisting due timers front, component ticks/output, up to128 updates, each drains outputs/forwarded paste, newly due timers back. Updates dirty+body_dirty; redraw tick dirty only. ViewFn once next preparation. Exit closes ingress clears queued actions/timers.
- AppCx lends scene/components/routes/timers/theme/global keys/paste; component removal also removes paste interceptor. Global key fallback only after native component/focus routing Ignored; forwarded paste deferred and bypasses interceptor.
- Host owns strong Arc mutex runtime+backend+authoritative frame+candidate+receipt+state/content registries+environment. Environment weak hosts, global source registry, FIFO/coalescing queues, retry block and receipt waiting sets, edge latch/epoch. TS broker schedules microtasks/receipt poll; Rust membership authoritative.
- Desired root preflight state/content duplicate bindings then desired revision/pending epoch; no immediate presentation. Host flush candidate captures state overlay/content plans/epoch, prepare, submit, receipt, content+state+frame promotion under environment completion authority; newer pending remains queued. Failed candidate never replaces old visible readback; receipt failure marks physical_sync_unknown; recovery includes History synchronization.
- Deferred control retirement waits for successfully reconciled SceneHost mount graph. Controls weak host, own mutex state; History uniquely strong host.
- explicit close disposes state/content/retained scene, handles receipt errors, restores terminal; exit final frame -> native rows/terminal final positioning -> restore, success-only closed cleanup.
- Test-driver32 input budget/8ms presentation interval are NOT production constants. Source cache2 + prefix cache2, persistent16KiB pages, branching/leaf16, annotation treap; semantic cache excludes theme/width/delivery paint keys.
- Exceptions need register: mounted poisoned controls spacer/ignored; manual unsafe Send/Sync host/environment safety assumes mutex/callback discipline; checked exhaustion panics.
- Native error phases/codes: BACKEND_IO_FAILED, BACKEND_NOT_READY, LAYOUT_DID_NOT_CONVERGE(nonretryable), HISTORY_TRANSFER_FAILED(retryable), FRAME_PREPARATION_FAILED, CONTENT_SCHEDULER_FAILED, SOURCE_WAKE_FAILED, HOST_LOCK_POISONED, INTERNAL_INVARIANT. Automatic failed epoch blocked to avoid spin; explicit barrier force retries; per-host isolation.
- No checks executed by scout; tests inspected only. LOC approximate not exact. Generic observability weaker than TS wake counters/trace256.

## Findings requiring reconciliation/source checks
1. wait_for_output host.rs1513–1535 polls local runtime/terminal but not drain_pending_for; normal TS broker may deliberately cover Source wakes. Verify native-only contract and actual poll_terminal effect before defect label.
2. HostHistory strong Arc + TuiHost Drop close only strong_count1; HostInner Drop cleanup but no explicit restore. Inspect TermwizBackend destructor before classifying.
3. host.rs1347–1361 method called HostHistory::set_history in report may actually TuiHost::set_history (report likely naming error). Deferred invalidation vs push/freeze/discard synchronous render; inspect whether marks environment pending.
4. Mounted poison fallback masks failures; inspect intentional policy/current handoffs.
5. unsafe Send/Sync not made sound merely by mutex serialization when non-Send state has thread-affinity; inspect actual NAPI task placement. Potential consequential safety claim, not accept scout justification uncritically.
6. Generic Rust App public coverage discrepancy only relative to authoritative current contract; no issue solely from deprecated docs.
7. Controls sync render vs root defer; establish supported TS retained transport path before claiming externally inconsistent API.
8. Content application module depends directly on presentation layout/textgeometry/physicalrows; map coupling without prescribing V5.
9. History mutation accepted before frame failure seems desired/visible design, verify.
10. Cleanup retry can coexist committed frame; check TS caller reporting.

Evidence index is report §10. Related reports02,07,10,15,16,18,21,23,34,38,40 should resolve cross-boundary questions.
