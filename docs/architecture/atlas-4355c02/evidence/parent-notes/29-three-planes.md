# Parent full-read notes — 29 Three planes
Fully read1–455;456–910;911–1365;1366–1811.

## Core synthesis
Three RETENTION forms distinct from three MUTATION planes:
(1) TS retained execution scopes/children/props/semantic slots/committeddeps WIP.
(2) TS accepted-native knowledge: WeakMap node→generation/ref, root desired/visible leases, attachment/resource keepalive, path/style hints.
(3) native authoritative retained data: environmentNativeViewRuntime weakNodeId/ref+strongleases, host desired/candidate/visible state and registry snapshots/projection/backend.
Mutation planes structure (semantic topology NativeRef), state (native values fixed envelope), content (source revision bytes/annotations directFFI + NAPI controls) converge HOST FRAME BARRIER, not one shared retentiontable.
TS publication interface prepare/commit/abort owns no NativeRef; boundary supplies implementation. Nested protocols evaluation allprepare then childfirstcommit. Desired native acceptance can precede/occur inside TS scope commit, hostvisible later. Do NOT present enumerated §8.6 as strict chronological three commits; composition may pathologicalthrow after individual desired publishes. No general atomic rollback on commitphase.
Tui boot/native environment+host/runtime+bootstrap frame; JS broker host registration; close unregister→native content cascade→attachments/stateclear/registry invalidate→ownedhandles/execution→rootboundary→nativehost. Scope disposal projection/deps/slots/recursivekeyedchildren; source environmentowned not Tui.
Native structural runtime fields nodes weak, node_refs, paged slots, pathnodes/keys, builders/edit_txns, style tables, generation. NativeViewSlot (internal cache slot, NOT public ViewSlot control) has NodeId/WeakView/optional strong View/jsleases/kind. Weak ref hint NOT ownership, transient helper promotes lease before release. NodeIds safe highbits; disjoint ref ranges no recycle lifetimegeneration; page highwater retained.
Native host frame/candidate/receipt/captured epoch+structrevision+contentdirty/stateplan/contentplan+physicalsyncmarker. Desired→pending→capture/prepare plans→submit→receipt success commit exact candidate; newerdesired stays pending. Error discards candidate, oldvisible authoritative, terminal physicalsync can be unknown→full recovery. Thus Sourceaccepted ≠ projected ≠ submitted ≠ visible.
State TS semantic HandleId + resource desired/visibleleases vs Rust record mutable/currentArcversions, dirty,demanded desired/visible/inflight, candidateoverlay pins oldArc. Parent read registry90–195: mutation demanded snapshots immediately into 'committed' version table, capture union demanded sortsIDs and only touched/new/missing; 'committed' table NOT necessarily visibly painted state. Overlay frame pins old across latermutation; clean referenced table safe because frame resolution synchronous.
SourceFFI fixed4identity words,6u32result; acceptedmutation/wakefailure later independent; controls same registry; perenv wake fanout coalesced; nativehosts weakqueue authoritative JS broker doesn't mirror subscriptions.
Native source control status cleanup precedence one JS error lane distinct Rust causes.
History wholecandidate extraction combines body/currentstaticlive/prospectivehistory statecontent attachments, so History changes can invalidate state/content without body change.

## Exact routes and corrections
Report §5.1 wide axis 'builder for larger' overgeneralized. Parent independently read native-view-abi.ts194–303: specialized tryRetainedAxisCreate uses0–4 fixed then createAxisWithBuilder perchild push. BUT production retained-dag.ts515–564 materializeAxisNode uses0–4 fixed then ONE viewAxisCreateBuffer on scratchpairs. Keep route distinction and caller reachability41, no blanket builder route.
Exactroot onehostRenderRef no semantic descent, oncachemiss promote byNodeId and retry once; desired root install differs generic transientexactrender. ensureSemanticNative hint→txdedup→eligibleNodeIdlookup ceiling→cycle→derivation→directchildrenfirst. Path/style caches gen scoped. Text≤4 cstring ifNULfree elseUTF8; >4 wordbytebuffer; not direct C dispatch, generated NAPI calls.
§5.1 builder rerender selection isn't 'new producer identity': calling functionform again re-drives existing scope even same closure. §2.4 restoreproducer catchesall including pathological; ordinaryfail preserves old but committhrow partial exception.
§2.6 disjoint ref ranges for path/build/edit vs ViewRef are checked; styles own domains.
§8.6 failure stage1/2 preservesprior only eval/prepare ordinary; must qualify commitphase invariants.
§6.3 says five structures lists SIX (records/committed/dirty/desired/visible/inflight); inconsequential prose correction.
Affinity/magic guard NativeViewRuntime raw helpers insufficient pointerlifetime proof (17); don't claim invalid pointer safe solely guard.
Broker64 bounded attempts but errors throwPending EACHreport; not64 silent retries ofsamefailure.
Static tests encodecontracts; noneexecuted here. Extents approximate and inconsistent e.g tui.rs1450 vsactual1979; notcensus. Other specialized reports close most gaplist.
