# Parent full reading: 39 slots, controls and animation
Read all 1902 lines in ranges 1–480,481–960,961–1440,1441–1902. Static source evidence, no executed timing/test claim.

## Integrated facts
Factory/native registration, seed materialization/adoption, semantic component occurrence, native mount activation, and visible frame are separate lifecycle stages. Local HandleId, semantic NodeId, native ComponentId and NativeRef are FOUR coordinate/identity domains (report §3.3 groups last two misleadingly).
Slot/pane direct and builder ownership share retained materialization; builder roots subscribe in shared retained execution. Static boundary lease, transient transport leases, native strong View clones and attachment leases are separate owners.
Animation frames installed by scalar 1–4 refs or reusable buffer >4; no per-tick TS callback. Mounted slot nominal tick is 16ms, animation interval checked within native tick. One elapsed-time call advances at most one frame, not catchup; no normative intent claim necessary. Cycle-boundary same-interval existing multi-frame replacement changes pending vector only until wrap. Different interval/small existing vector installs immediately.
Native ScrollPane mode survives replacement with extent reset and clamp; TextInput buffer/Unicode/focus/key/paste remain native, output is typed local routing -> opaque host route ID and payload, not JS key handling. Deferred component retirement waits successful mount reconciliation. Factory handle ownership and output facade owner distinct.
Static root boundary/currentView are NOT updated by animation or stopAnimation. Native strong frame/current Views keep values alive; old boundary root still retained. Attachment validation for animation/stop is prepare+abort, NOT committed attachment binding replacement. This seam needs grouped lifecycle evaluation, not assertion leak or bug solely from duplicate owners.

## Parent independent checks / corrections
Read full TS view-slot.ts and Rust host.rs239–462, plus retained-dag.ts1641–1745,1911–2010. Confirmed animation/currentView/boundary divergence and native tick behavior.
IMPORTANT report §4.2/§5.4 claims old native root remains on install failure TOO STRONG: Rust set_view mutates state.view/frames/revision BEFORE invalidate_host()->advance_and_render can fail. Boundary catch unwinds acquired leases and preserves old TS bookkeeping, but does NOT undo native slot mutation. Same general acceptance-vs-visible failure seam as reports30/31, not atomic rollback of all control state. Native callback returns true only after successful call; throw can follow native desired control change. Preserve failure phase distinction in synthesis.
Boundary publishPrepared DOES call installRef even exact same root; no evidenced same-identity setView no-op after animation.
Report stopAnimation(object) stale addon member is source-audit lead (no public supported route), to reconcile41; optional seed double-spacer occurs internal raw construction only, public factory supplies initialView, do not inflate.
No production edits or test additions.
