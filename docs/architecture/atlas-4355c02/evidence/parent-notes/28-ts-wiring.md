# Parent full-read notes — 28 TypeScript wiring
Fully read1–410;411–820;821–1230;1231–1640;1641–2010. Detailed source inventories retain report as reference.

## Integrated graph / source ownership
Two workspace packages @iyon/tui and private consumerfixture (workspace dependency only). Root workspace duplicates package exports '.', './testing', './native-stage'. No product packages here. Intended imports public root+testing; internal benchmark/tests relative transport imports not consumer API proof. Stage CLI third export means report's 'only documented secondary export testing' too broad.
API groups semantic View/Scene/ViewState; style/theme/selectors; content value/Source/Funnel/Port/Connector; controls/handles; generic trait contracts. Value exports vs type exports distinction: FrameworkHandle type only, ViewSlot/ScrollPane types via factory. Runtime internals not root exports.
View ↔ composition cycle controlled active-context module no View, raw construction flag, type imports. Semantic-node immutable IR/NodeIds+WeakMap attachments/derivations/persistentsequence sidecars central cross-layer seam. Composition owns scope table, child keyed/positional WIP, State subscriptions, schedule; publication interface carries View only.
runtime/Tui owns host, retainedruntime, rootboundary, scene/history sideband, attachments, errorchannel, handles and one environment registration. Environment singleton shares resource registry+broker. API content imports runtime/environment not Tui; API controls deep transport coupling. Resource registry plane-neutral metadata ownerhost/env/accepted kinds/weak entries; leases strong while prepared/desired/visible.
Structural transport semantic DAG hints+leases; generated NAPI session/calls; State generated fixed envelopes over NAPI; Source bulk direct FFI same artifact controlsNAPI; no complete-object fallback. Trait ComponentAdapter async stand-alone tests != synchronous defineView scopes and no automatic native registration.
Three identity scopes: JS HandleId not native; NodeId immutable semantic; execution parent/key/position; NativeRef lease/runtime generation; native ComponentId, host epochs, source rooted IDs.
Direct Scene takes root ownership after success and disposes producer; canonical builder persists root identity across closure replacements. Scope evaluation→allprepare→childfirstcommit; abort restores prior deps/output/dirty obligations but no autoretry; pathologicalcommit not rollback. Equal props Object.is ownkeys; State reads only activeeval subscribe; writes reject during body.
ViewSlot/ScrollPane same Tui executionruntime for builders, own rootboundary and attachments; direct takeover prepare before disposebuilder; pane scroll state separate. History attachonce and caller/tui ownership; Sources environmentowned may outlive host, Connector/Port hostowned.
Root/attachment desired visible superseded ledgers protect receipt flights; actual visible authority native host. Theme sidecar reset finally AFTER native theme set. Broker automaticerror channel vs explicitthrow; timer polls receipt not microtaskspin; cleanup aggregates/retries.

## Evidence qualifications / reconciliations
Report correctly states TextContent.render raw View.text and minimal TS Projection wrappers. Preserve stronger19 parity warning: Markdown metadata not native Markdown and TS Smooth not native pacing.
§4.6 narrow Header/single1000child invalidation without new render matches fixture; broader keyed reorder assertions elsewhere require explicitconsumer.renderApp (25).
Stage validation occurs AFTER copy (26); same artifact invariant yes, failure not oldartifact preservation.
Strong 'fixture proves' throughout means source assertions, NOT executed during atlas. LOC estimates not census, generated counts imprecise. root API inventory preserved via19.
Cleanup retry generality qualified21 shrinking pending.length loop may silently leave last dependency; need group40 reachability review.
Text-state native ref 'physical value' should say semantic native View, physical cells only at paint.
