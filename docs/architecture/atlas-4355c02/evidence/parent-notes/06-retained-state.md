# Parent reading notes — 06 Retained state
Fully read1–490,491–980,981–1470,1471–1962. Static scout evidence, no new independent source checks.

## Inventory
- Ten files~2001lines/1600prod/400inline tests. capabilities,capture,damage,effects,geometry,occurrence,presentation,record,registry,mod.
- Public GeometryAlignment; native-host gates geometry/presentation patches/properties, HostViewState public wrapper application/view_state.rs. Registry/record/snapshot internal.
- StateNodeKind exhaustive Text,Spacer,Row,Column,Grid,Hanging,Container,ClampRows,RowViewport,ContentHost,ComponentSlot. Slot no physical presentation box; gap Row/Column/Grid only; horizontalalignment Text only, vertical Row only. Unbound stored geometry deferredvalidation mount.
- OccurrenceBox embedded LayoutNode retains optionalstateid/nodekind/base+effective width,height,gap,alignment,decoration,stylestates. Not wrapper nor parallel tree.
- Geometry patch nullable bounds/borderedges; sizes Fit/Fill. null minimum0,maxu16MAX; borderedges creates plain border or removes; clear revealsbase.
- Presentation fields nullable foreground/background/bordercolor/style/glyphs/styleRef, TextAttributeSpec. directstyle merged,themedstyle replaced,explicitnull resetsdirect; textattrs appliedlast. Border presentation only modifies existing border. generic style-state BTreeMap overlays inherited base.
- record revisions overall/geometry/presentation (stylestates usespresentation) saturatingu64. prospective geometryclone validates beforecommit; noops norevision/effects; stylekey/value nonempty.
- HostInner owns boxedrecords and Arc immutable committedversions; wrapper id+Weak host, no perrecordlocks. id=(hostid<<32)|local; host<=0x1fffff ensures JSsafe integer; localu32 monotonic, no reuse.
- records,committed,dirty,capture_epoch,nextid,desired/visible/inflight sets. committed is latest immutable logical version NOT backend-visible state. mutate demandedrecord replaces Arc; dirty allaccepted evenunmounted. create nosnapshot/no dirty. capture sorts demandunion, touches dirty/new/noncommitted; clears demanded dirty only. unmounteddirty persists.
- candidate overlay(epoch,demanded,touched) lookup touches then committed; ResolveSession pins demandArcs. Unbound pruning removes committed not record; candidate Arc survives. Candidate validation sets inflight; receipt success commitvisible,failed clearsinflight preserveoldframe. hostdesired validation resolved components+History detectsduplicates/cycles/missingoverlays.
- disposal rejects desired/visible/inflight STATE_MOUNTED; unknown/repeat idempotent; wrapperGC notdisposal, hostowner teardown clearbindings then disposeall.
- state-only scene: structural/content combined escalation; invalidate state-to-root caches, overlay snapshots; presentation apply OccurrenceBox and subtreepaint; geometry smallest safe allocation/dependencyfrontier patch else retainedroot relayout. Missingindex -> conservativecacheclear/fallback.
- effects15bits, only geometry/intrinsic width/height queried; remaining bits expressivemetadata, NOT independent dispatcher. Purepresentation includes subtreepainting irrespective declared selfbit.
- measurekey geometry+presentationrev even presentationmetricneutral; state fastpath avoids measure. State painttargets sortedphysicalnodeid.
- Damage clipped/touchingmerged >64rects or >=halfviewport -> full; zero viewport fulltrue emptyrect. geometrydamage compares rect/content/clip/occurrence oldnew. Actual Termwiz lower whole surface; DamageRegion not consumed there.
- testsinline capability/capture/damage/presentation/noop/registry, hostscene failedreceipt/retainedstate; TS perf13_b/retainedscene/inputvalidation; staticnotexecuted. counters12 state acceptance/noop,invalidation,paint/damage,geometrypatch/relayout/propagation.

## Issues / reconciliation
- Report repeatedly relegates PERF13 to historical-only authority; correct framing in integration: scoped unsuperseded handoffs oracle; descriptions source-authoritative but deviations need evaluation.
- Explicit fields vs PERF13 PropertyDescriptor table potentially deviation; group with transport/ABI/effect classification. Not automatically justified because implemented.
- Damage unused backend vs required contract; report alone proves architectural fact not normative violation. Crosscheck15/41/44 and handoff.
- Effects narrowconsumption may safely conservative paint more; do not repeat question plainpresentation selfonly paint as bug because actual subtreepaint.
- Snapshot concurrency distinction key; report claim candidate versions committed onlywithframe imprecise: logical committedtable advances atmutation, visibleframe onlyreceipt.
- Noop captures allocations/demanded sort, semantic statepath reconstruction, saturatingrevision risks observability/lowpriority not proven defects.
- Single occurrence/state intentional current constraint; clarify not unsupported multiuse feature bug.
