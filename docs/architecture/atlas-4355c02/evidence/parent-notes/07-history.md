# Parent reading notes — 07 History
Fully read1–450,451–900,901–1301. Independent checks below.

## Inventory
-11files~2525LOC; model407,projection1100,native559,frontier104,trace141 +boundary/error/id/layout/unit/mod.
- Generic root ordered semantic History(VecDeque units) + widthprojection + irreversible nativefrontier, NOT arbitrary-scroll controller; ScrollPane separate. Units retained until live-taildiscard/nativefrontretirement, viewport selection notculling.
- HistoryUnitId NonZeroU64 processatomicmonotonic; FlowBoundary Default/AttachToPrevious controls currentunit predecessorgap. LayoutInsets/gapu16. Error UnitNotFound,UnitNotLive,LiveMustRemainTail,FinalViewContainsComponent nonexhaustive.
- push classification componentidentity=>Live elseStatic; ContentHostwithoutcomponent Static but provider mutable. push Result alwaysOk currently. freeze live->staticfinal prohibitscomponent, permitted non-tail, no native transfer implied. discard only livetail. layout equalnoop/allheightinvalid.
- Width+key perunit RefCell cache: Static(ViewId); Content(rootviewid,providerprojectionrev); Live(viewid,reachable componentrevisions). aggregateheight +stalecount. Missingmeasure uses fresh LayoutCache each call; cached outerunit major reuse.
- state/content views include component-bearing entries for hostresolvedvalidation. HostHistory prospectivevalidation beforepush/freeze; replacementclearoldport/bindnewport updatesdesired. HostHistory strongArchost (unlike Weak HostViewState); potential lifetimeissue pending broadercoverage.
- Scene body measured first,History gets residualterminalheight, merge visuallyHistorybeforebody and mountorderHistoryfirst evenzeroheight. Separate semantic/native revisions wrapping, nativeonly can retainbody.
- Plans everyunit,resolve alllive for dependencies, provider history_view root; flowtop/gap/unit/bottom. FollowEnd backward vs NativeFrontier forward; cachedselection iff retained geometry (no frozenstatic/no priorphysicalrows) and no protectedopencontenttail. Flexiblelive boundedrowviewport. Physicalfrozen overlay clippedHistorytrack.
- Native transferbudgetoverflow, repeat untilpressurehandled, semanticzero-rowretirement progress. ordering topPadding,leadinggap,frozencontent,frozenstatic,frontunit. Live front blocks cannot skip. plainstatic compiled,content providerstableprefix,exact acceptedprefix stored.
- FrozenStatic/FrozenContent/SpacingTransferState frozen remainders retain exact PhysicalRows acrossresize/layoutchange, NOT regenerate. Semanticfreeze different.
- Sink Ok(k) exactprefix; contract Errzero accepted but defensive unknownsync onerror/invalidack. no logicalrewind to avoidduplicate irreversiblephysical writes; hostsuccessfulrecovery commit clearsmarker.
- retirefront popsunit,recordsretiredid,lastnativeunit,resetspacing/unit; outeradapter drains retirementcallbacks even latererror. contentcallback commitsrowcount, stripsacknowledgedinsets, retire deletesport/connectors.
- Content finalizedprefix only stable/delivered; sealed Smoothbacklog notcomplete. zerorowsincomplete blocks; zerorowscomplete retires.
- Traces env IYON_HISTORY_TRACE OnceLock atfirstuse; projection/transfer/resolvepressure. counters unitsexamined/measured/cachehits. Testsource rootheight/duplicate/mountzeroheight/physicaloverlay; contentexactprefix/sinkfailure/widecell/resize/smoothretirement. noexecution.
- Root history attached lifetime/native controls outside recursivehistory model; publicTS layout/push/freeze/discardLive/setLayout.

## Issues / reconciliation
- Handoff historical-only framing correction asotherreports; unsupersededapproved docs normative.
- NativeSync recovery clear itself not verification; must sourcecheck surroundingbackendprotocol before assurance.
- Report only root ContentHost cachekey and componentrevision Live key leaves potential nestedcontent or retainedstate geometry not represented. Parent independently read projection/mod.rs151–260 CONFIRMS exact keyconstruction uses view.content_attachment_id() only; no state revisions. Scenehost text search no invalidate_all_layout call. Need broader kernel invalidation/ancestor cachepaths check and reachable reproducer; not establisheddefect yet.
- Parent independently checked host.rs811–850 HostHistory strong Arc confirmed; ownerdead/close semantics in40/18 needed.
- No-tailfreeze/sourceclear absence directtest not a defect. No rootuser-scroll and semanticculling are designcurrent facts not missing promised features.
- Unit IDs report stable whiledeque; callback/oldhandle invalid afterretirement.
