# Parent reading notes — 08 Semantic content
Fully read1–475,476–950,951–1425,1426–1900. Independent source checks/errata below.

## Inventory
- content tree~8kprod/5.5ktests estimates notmeasured. Largestmarkdown1433,style556+466test,render/mod546/block523,blockIR582. Separate content/diff typed model395+163test/render88+190test, content/text/diff230+83test.
- Private modtext -> internalaliases, selectivebindingexports; whole authoringRustIR not externalpublic. Renderer<Input> trait test-only; TextRenderer productioncrateprivate perconnector.
- TextContent Raw(RawText) orBlock; noInline/Literalroot. Raw pageArc/startu32/lenu32, textbasedEq; RawTextnew unchecked usize->u32. Block/Inline immutableArc ptrEq, caches retainowners topreventABA.
- BlockKind Paragraph,Heading1..6,Quote,List,Code,Table,Rule,RawBlock,Container. Lists bullet/orderedstart/style/delimiter/tight,taskoptional; zeroordered allowed. Tablescaption,columnalignment,headercount,spans,annotationrow/cell; construction validates overlap/schema/header/spanbounds, computeslogicalcolumns.
- Inline TextRun/SoftHardBreak/Imagealt/RawInline; canonicalMarkSet sorted dedup Emphasis/Strong/Strike/Underline/Super/Sub/SmallCaps/Code/Link<=1. InlineContent Arcordered.
- TextRun textArc,Exact/Derived(rootrange)/Synthetic,annotations,optionalStyleRef semanticintent butpresentationtyped. exactconstructor lengthcheck notbyteswitness. splitUTF8 exactsplitrange,derivedretainwholeparentrange,syntheticunchangedmetadata. LiteralText runarray.
- annotations sortedArc tags(namespace/name),propskey->Bool/i64/Text/TextList; nonemptywhitespacefree; replaceprop/idempotenttag. Origins builtinsMarkdown/plain/ANSI/diff+custom through iyon-tui:origin ordinaryprop; recursive stamp except code/raw literalbodyruns; renderer inheritedcontext stilloriginfacts.
- validateprojection envelope +Raw solevalue byteslength +nested provenance containment/exactlength, noexactbyteproof. RawDomain witness providesconstructionproof.
- visitor reaches allnested lists/tables/captions/images/literals. RewriteProjector preservesenvelope andvalidates; no-op persistent blocks/inline/vectors retainidentity.
- Raw sourcechunks -> RawTextpages -> contiguousrawdomains; structuredboundaries hardbarriers ALLprojectors. RawDomain singletonborrows page; multipage lazyOnceLock assembly, newline scan pieces, prefix/suffix narrowwithoutfullassembly.
- Plainstateless paragraphperdomain hardbreaks/exactruns/origin. Markdown pulldown GFM/CommonMark options,frameeventstack,nesting/source/parsererrors; exactifparserbytesmatch elseDerived. rootlist emits peritemListblock for closeditemstability; nestedlists staywhole. RaggedGFMtable pads/drops toschema; live-table stabilization policy notgrammar holds rawpipeparagraph until followingblank/nonpipe orseal.
- Markdown requiredrestart/checkpoints/cache sourcebase/stableend/spans/referencecontext. cache exactstableprefix/suffixparse/referencefullreparse; stabilizedproof reparse. sealedstable=end. cachevector lifetimeconnector notexplicitbound.
- TextDiff lineoriented paragraph; @@ entershunk,addition/deletion excludes +++/---,contextspace,no-newline/meta; syntheticmarker/exactbodyhardbreak; permissivemalformed retainedmetadata. Stateful completedinlines/end/hunk, no typedhunkmetadata.
- ANSI state SGRcolor/attrs/OSC8links, consumesunsupportedESC/cursorcontrols; incompleteescapes heldunsealed,droppedsealed. completedprefix state,sourcecontinuation reset. Generic styles retainedStyleRef.
- TextRenderer lowers paragraph/headingtext,quote/list hangingmarkers,code label+literal,tableGrid,ruletext,rawliteral,containercolumn. RenderContext completeancestorrolepath/origin/list/task/tablesection/lang/format, cachedsemanticfacts; __iyon_tui.text StyleRef resolvesTheme later. TextSelector ordinaryStyleSelector facade,notseparateengine.
- TextRenderPolicy structuregap/softbreak/tabletracks/taskmarkers/codelabel/wrap separateTheme colors. imagealtfallback notterminalimages.
- Blockcache4 clearall, keyptr+fullinheritedcontext ownsblock/View. Rawcache1024 pageptr/start/len retainspage; edgecache2048 childViewId/gap/predecessorlist; semanticsequencecache4 persistentitem/edge/child/View prefixreuse tailrebuildshortening.
- App ConnectorExecution perconnector parser+Smooth+renderer;deactivate dropsall. lineage sourceid/sourcegeneration/contentgeneration resetsparsers NOT renderer/deliverypolicy. semanticcache2 keysourceid/generations/revision/baseend/sealed/funnel/hyperlinks excludeswidth/theme/tick/wrap. TextProjectionKey adds widthwrapdeliverytheme/finalized/physical. semanticcompile lowersView->ViewCompilerwidthlayout->rows; layoutonly route. Puretick advancesSmooth no reparse; finalizedstableprefix separatelysealedrenderedproof,notcutopenrows.
- sourceannotations postparse rewriter exactbyte/proportionalderived/syntheticskip; directrewrite_inline split keepsfirst but actualoverride vector expandsall.
- Direct typed DiffHunk rangeoffset/1basednumber/kind/termination/count/overflow/sequentialvalidation -> directlowerdiffhunksView via nativewordsbytes; bypassTextRenderer, sharesdiff.*Theme only. Distinct validusecases not duplicatefallback automatically.
- Static test coverage extensive, notalltestfilesfullyread byscout, noexecution. parserworktestutil, semanticrebuildcounterapplication, noownbench.

## Independent checks / errata / issues
- Read source.rs366–407,content.rs56–90,provenance.rs16–90: exact_runs splits sourcewitnesses at retainedpage boundaries BUT resulting TextRun::exact(&str) constructs fresh Arc<str>. Scout 'page-backed exact provenance' must NOT become zero-copysemanticruns claim. RawText/RawDomain retainpages; runs carrycopiedtext+range (no sourcewitnessstored).
- Read markdown.rs141–161 confirms InsufficientRestartContext iff input.source_base()>required. Report4.3 step2 inverts wording ('required ahead ofinputbase'); laterfailuresection correct. Recorderratum without alteringoriginal.
- Directprojector samebasereplacement hazard needs09 contract andapp lineage guarantees; distinguish internalcontract vs publicTS projectionfacade. No externalRustAPI automatically.
- StyleRef dependency genericsemanticstyle notproductpolicy violation, possible extractionseam notmustfix.
- TextDiff and typedDiff intentionallydifferent fidelity; compareAPI-H oracle before classifyingrequiredconvergence.
- sourceannotation proportionalderivedmapping/directsingleinline droppingpieces userobservable? groupcontent followup with source-based conditions.
- RawTextu32 limit/cachebound risks require reachableinputownerlimits beforedefectlabel.
- Verified diff.rs156–230 classification matchesreport; hunkstate neverreset bymetadata infunction, permissive lines beginning+++ alwaysmetaevenlegitimatehunkline; report expectedpermissive nottypedvalidator. Potential formatfidelity question notyet issue.
