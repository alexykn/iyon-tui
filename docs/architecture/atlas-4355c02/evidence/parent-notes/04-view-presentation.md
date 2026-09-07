# Parent reading notes — 04 View/presentation
Fully read1–450,451–900,901–1300,1301–1599. Static source testimony, parent source-check pending.

## Synthesis inventory
- presentation private module, hidden native binding facade; api semantic records not public Rust builder system. factory canonical in-crate production construction; ir semantic retained representation; wrap physical-aware text seam. layout/paint excluded report04.
- View Arc<ViewNode>, new process-local ViewId on EVERY semantic mutation even unique. Clones retain identity; semantic equality ignores id+cached flags; unchanged children shared. Not future V5 occurrence identity.
- ViewNode id,flags,state_attachment,content_attachment,width,height,decoration,style_states,style_facts,kind. Kinds Text/Column/Row/Grid/Hanging/Container/Spacer/ClampRows/RowViewport/ComponentSlot/ContentHost.
- ViewFlags aggregated component/state/content presence allow short-circuit. PersistentSeq32-way leaves Arc slices, branches Arc children+cumulative sizes+aggregates. Set path copies, split/concat/splice share unaffected chunks; native axis/grid/batched edits don't flatten.
- Final factories assemble ViewNodeParts once then from_node once; native common patch batches modifiers, normal semantic modifiers each new outer root. Attachments preserved; map_text uses Arc::make_mut payload.
- TextView Arc spans,wrap,align,cursor byte offset. TextStorage inline<=12bytes,PageSlice Arc<NativeUtf8Page>,SourcePage Arc<str>,Owned String. Page ranges validate UTF8; mutation materializes; source render retains pages beyond cache eviction.
- StyleSpec sparse inherit vs plain explicit false attrs; overlays only Some; StyleRef theme key+local override. StyleStates inherited, StyleFacts self-only/cleared descent; facts win, then states. Sorted bags binary lookup. Positive focus/focusWithin + caller predicates. Two different state systems (style bag vs retained ID).
- Border eight glyphs each exactly one grapheme+one cell. Overflow none/ellipsis/footer semantic; layout/paint responsible.
- Grid construction eager row-major occupancy/spans/implicit tracks, persistent cells and origin-coordinate index. Setter targets origin not interior span. Zero spans panic.
- ComponentSlot holds ID no resolved child/presentation state; scene overlay expansion separate. ContentHost holds port ID no bytes/projection/connector/rows/scheduler.
- State attach validates positive+kind; raw IR walker cannot expand component slots; Scene walker can. Native content attach only ContentHost positive ID.
- Native axis compact word low byte kind/high16 value; kinds1content,2maxcontent,3fixed,4minflex,5maxflex. Set track0 preserves, construction0 content. Decode inserted tracks before range validation. Native path steps no View retention; typed parent/selector checks, batched all validate before rebuilding, empty pointer-preserving, text patch depth cap128.
- ContentProvider seam receives IDs,width,window,ticket,destination; registry owns lifecycle. Ticket {port,optional connector,width,projection revision,projection identity}; measurement size/complete/projection/metric/paint revisions + connector/projection identities. Width not identity (equal-width connectors).
- Dirty source/delivery/width/selection measure+paint, presentation/viewport paint-only. Trait history hooks defaults explicit no-op/None/originalclone; EmptyContentProvider zero metrics/no painting for no-mounted-port/test callers; investigate accidental production omission only with route evidence.
- wrap hardlines \\n, grapheme across spans uses first contributing style; stored terminal width authoritative, PhysicalStyle. Cursor valid UTF8 source offset snapped grapheme lead; invalid assert. Oversize grapheme nonfitting never split; input wrapping preserves source ranges/caret reserve.
- Perf ViewCloneCalls,ViewNodesConstructedRust,PersistentSeqNodesAllocated/BranchClones/LeafClones,TextFlowMeasureCalls. Source tests cover one final root/equality/sharing/grid/page lifetime/style/wrap. No runtime validation.

## Reconciliation candidates
- ThemeKey documentation interning vs Arc constructor only: check native atom table before issue. Neither equality dedup nor global interning proven here.
- ContentProvider stale ticket concrete behavior in application/content; check fail-closed/retry.
- Report rough LOC internally inconsistent: summed table production/test doesn't support summary575test lines (ir394+wrap287+grid263+style95+text74 already1113). Use measured counts not scout approximate totals in synthesis.
- 'ContentHost' listed as presentation::ir type in appendix likely enum variant not standalone type: exact symbols inventory needs parent correction.
- native_patched is one final PATCH root beyond base, not necessarily one root for whole ingress; preserve performance route nuance.
- Historical/source authority wording must follow narrowed oracle rule, not generic current source wins for intended contracts.
