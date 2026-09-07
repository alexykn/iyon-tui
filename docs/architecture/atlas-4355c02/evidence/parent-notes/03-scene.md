# Parent reading notes — 03 Scene
Fully read lines 1–450,451–900,901–1300,1301–1640. Static scout evidence only; independent source verification pending.

## Inventory for synthesis
- Scene is private external Rust surface: body + optional History; preserves authored body separately from fill-normalized layout_body and synthetic layout_root. Need check claim EVERY visited node Fill vs root only in source (report invariant unusually strong).
- Body measured terminal width, height clamped viewport; History receives remainder, zero-height still mounts semantically. Root isn't ordinary View. History-first mount order. Separate branch products retained intentionally.
- ResolvedRootScene {scene,body_scene,history_scene,history_components,body_view,history_overlay,history_overflow_rows,history_height,body_height}. Frozen overlay is physical and root-specific.
- ResolveSession preserves semantic View with component snapshots in ResolutionOverlay, not replacement tree. MountGraph, capabilities, indexes resolved alongside. Missing/Duplicate/ComponentCycle errors; flags short circuit component-free branches; sorted reachable (ID,revision) dependency set.
- ResolutionOverlay components + shared Arc state snapshots; ResolvedScene equality ONLY view+mounts, excludes overlays/capabilities/path indexes.
- content_paths,component_paths,content_path_components track Arc-clone semantic paths/reverse component dependencies. State/content target walkers expand slots and detect cyclic View graph/duplicate IDs; content only ContentHost attachment.
- LayoutSynchronizer separates allocated-size and full-content-extent callback delivery, removes vanished capability/ID records. Dirty callbacks force full authoritative rerun; MAX_LAYOUT_PASSES8.
- SceneHost owns retained StableScene, graph/capabilities, focus/ticker/outputs, caches/last_surface, invalidations and native pressure; RunningApp owns actual component values and Scene; HostInner owns receipt-visible frame transaction.
- Initial full resolve body measure/history project/merge/layout/callback converge/mount-focus-tick/paint then candidate receipt. Same-shape component update can patch, otherwise full retained layout; filter invalidated descendants.
- History-only projection reuses body; changed topology/geometry may force full paint. State geometry patch only safe committed allocation else frontier/root. Content local body metric probe; changed width/height/completeness escalates, History content conservative.
- Theme metric-neutral layout-key omission, paint clear/full obligation. State combined with structural mutation clears both caches because ancestor keys lack descendant state revisions.
- Content epochs keep newer dirty obligations during in-flight older candidate; commit removes <= prepared epoch. Full_paint_pending avoids sibling stale cells after geometry move.
- Native pressure drains overflow budget (multiple units before resolve), retains body on progress, blocked pins NativeFrontier; unknown physical synchronization avoids duplicate emission; physical-progress then semantic blocker re-resolves. Irreversible side-effect boundary.
- PreparedSceneFrame Surface+physical overlay+damage+state bindings; state pins until receipt. Tick mutations explicitly invalidate IDs, not just registry revision.
- Tests listed source-only: roots sizing/order/zero-area; resolver errors/cycles/snapshots; local updates/cache ceilings/history/geometry/background/focus convergence/native pressure. Perf counters distinguish resolver/path/state geometry.

## Reconciliation / issue candidates
- STRONG contradiction to verify: report says failed frame cannot prematurely destroy component, but SceneHost graph updated after preparation and retirement may happen before BACKEND receipt. Need differentiate preparation failure from presentation failure. Same issue spotted01/02; cannot repeat blanket atomic lifetime claim.
- Source-check layout_body normalization exact walk semantics.
- Report 8.9 says no historical claim authoritative over source; restate this only for DESCRIPTION. PERF13/APIH/L1/PREV5 intended contracts remain oracles unless superseded/accepted better design.
- merge overlays can overwrite cross-branch duplicate state IDs; production native publication has separate complete target validation. Internal root resolution weaker; not proven bug. Check all actual production callers before classification.
- Internal visibility not inherently defect; no need keep scout external-Rust-access unknown once root exports inspected.
- DidNotConverge -> nonretryable host mapping must compare with retained retry obligation semantics.

## Independent verification / erratum
Parent inspected scene/root.rs1–100 and presentation/ir.rs1001–1040: layout_body normalization changes ONLY OUTER ROOT via nonrecursive map_node; report03 section2.1 invariant5 'every visited node' is wrong. Report05 root-only account correct. Preserve original scout report/hash; synthesis/reconciliation must override this wording.
