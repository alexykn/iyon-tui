# Parent reading notes — 05 Layout/geometry
Fully read1–475,476–950,951–1406. Static report; small independent check below.

## Inventory
- Geometry internal u16 Point/Size/Rect; AxisConstraint Definite/Unbounded, LayoutConstraints width_only/bounded; saturating halfopen intersections. Negative viewport translation via separate SignedRect then clamp.
- measure -> prepare -> place -> flat LayoutTree -> paint. Placement does not remeasure; tree no recursive View subtree, but cache DOES retain Arc<MeasuredNode> graph each holding View.
- MeasuredNode view,key,cacheability,component/scope,width capacity/rules,gap/align,decoration/style states,outer/core sizes,kind. PreparedNode Arc measured,outer/core offsets,complete,leaf/children/clamp/viewport; children localxy+Arc.
- LayoutNode ID local vector index,viewid,cacheability,OccurrenceBox,rect/content/clip,component,children,dependency vectors,style/content. LayoutTree root,size,complete,parent/component/state/content indexes,child_y_sorted. Text payload retained but not arbitrary semantic tree.
- ChildDependency4bits parent uses child width/height, child depends parent width/height. All conservative for row/grid/container/clamp/hanging; column fixed/nonfixed calculated; viewport intrinsicheight.
- Tracks gap normalization, fixed then content then flex minimums sourceorder; equal remaining rounds/caps, earlier remainder, no weights. Grid span requirements sort spanlen/start/original; content growth before flex, gaps counted; intrinsic/fill separate.
- MeasureKey view,component snapshotview,geometry/presentation/content revisions,width,intentSemantic/ForceFit. PrepareKey measured+height_bound. No scope/theme/parent. Component-containing branches uncacheable.
- LayoutCache current/previous measure/prep maps, promotion removes old adds current; two generations no capacity bound beyond working set. Full emitted tree still rebuilt each layout pass; no claim every paint-only frame relayouts.
- Unbounded width probe u16MAX then actualwidth; width measure,optional height prepare,origin0 placement,index/debugvalidate.
- Text width flow; ContentProvider measurement; row forcefit probes then allocated measurements; grid preferred widths,span widths,row intrinsic; hanging prefix/body/continuation.
- Scene root body measure/history measure/merge/final layout multiple stages; History view_height fresh local LayoutCache nested behind separate History unit-layout cache.
- State local refresh natural probe then bounded replacement at dependency frontier, can climb; content once measure committedwidth then fixedallocationpatch if size/completeness safe. Component patch size unchanged/topology constraints then geometry rebuild.
- Generic subtree patch validates preordercount/childcounts/dependencylength/view/component/state/scope identity, retains IDs/indexes. Component patch permits identity differences/rebuilds indexes.
- viewport stored unscrolled child coordinates + wide vertical clip, SignedRect negative skip applied traversal/paint. Cannot treat unsigned rect alone as physical coordinates.
- Missing component overlay panic, layout no Result; patch None/false escalate, completeness conveys nonfitting physical result; default absent content metrics concrete provider semantics need contextualizing.
- Counters MeasureNodeCalls/TextFlowMeasureCalls/PrepareNodeCalls/LayoutNodesEmitted/Paint*/ComponentGeometry*/state/content. Tests assert warm measure/prep0, full/row paint parity, clipped prune4096 siblings,oldgenerationsrotate. Source-only not run.

## Issues / reconcile
- Scope not in MeasureKey despite MeasuredNode.component_scope: possible aliased shared View cross-component wrong scope. Need check uncacheable slot behavior vs child cacheability and adversarial repeated same root semantics, not call proven bug.
- Generic patch compares dependency VECTOR LENGTH not bits; child_y_sorted not recomputed. Potential topology-stable geometry reorder causing paint pruning mismatch, need concrete reachable repro/source reasoning grouped ownership/layout investigation.
- Cache provider revision contract trust important; not bug by itself.
- Claimed per warm FRAME tree emission overbroad: only when layout pass chosen; paint-only route can reuse tree.
- Retained measured View graph ownership not violation of tree-only prohibition. Arc clones not deep allocation; do not turn memory concern into alleged recursive deep copies.
- No weighted flex/margins/fractional/overflowobject ordinary design scope, not defects or missing required APIs.
- Root normalization discrepancy with03: independently sourcechecked root.rs1–100 and ir.rs1001–1040: map_node ONLY outer root, NOT every visited node. Record erratum; exact roots Fill.
- Independently checked cache.rs1–65: key fields above correct; documented rotation ONCE PER HOST FRAME never between convergence passes. report warm/path summary should preserve.
