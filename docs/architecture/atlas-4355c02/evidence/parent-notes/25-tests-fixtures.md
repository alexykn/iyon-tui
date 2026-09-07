# Parent integration notes — 25 tests and fixtures

Fully read all1749 lines in four ranges1–450,451–900,901–1350,1351–1749. Static inventory; NO tests executed by scout.

## Model to integrate
- Public root @iyon/tui authoring/runtime + public testing subpath @iyon/tui/testing. Rust implementation crate private authoring, test-only/test-util helpers are not supported external Rust API. Consumer fixture uses workspace public imports, not packed artifact/declaration-only isolated client.
- AppHarness wraps actual production Tui forced headless; private WeakMap runtimeAccess bridges enqueue/native snapshots/clock. render Tui then advance0; input flush then enqueue; inspection flush+advance0 while local terminalExited false. Safeinteger/nonnegative/overflowcheck advance and JS clock increments only after successful native call. This does not prove native operation is fully rolled back if it advances then errors. exit sets localflag BEFORE underlying exit; close doesnot setflag. Lifecycle error wrappers asymmetric.
- Publicfixture direct Scene+history+controls and automatic composition defineView/state/key. Counters prove executionfrontier, not native algorithmic work.1000siblings single update counters only; offscreen child500 and layout/paint intentionally covered elsewhere. Two microtask hops used for flush completion. Builder-to-direct slot stops oldtrackedwrites; snapshot barrier can drive production work and doesnot prove autonomous interactive timing.
- renderRetained lowerlevel fullpublication helper materialize+hostRenderRef+finallyrelease tests incremental-vs-fresh samearchitecture, bypasses Tui scheduler/root publication/history/owner surface. Shared oracle bugs possible. Generated ABI Rust stubs validate wiring not semantic native implementation; versionmarker sync tiny test.
- Rust fakebackends/testdriver/input/errors/presentationreceipts/headlesssink/perf shared testlock, plus many inline testmodules. Testdriver/shadow test-only must not count as duplicate production routes.
- Root tests includes package+consumer but addonstage separate; CI stages native, generation/freshness/typechecks/declarations/binding/ownership and featurematrix. Exact baseline testpass not established here.

## Independent source checks/corrections
Read full src/testing/index.ts, tests/fixtures/native-host.ts, consumer tests/scoped-invalidation.test.ts.
1. Scout3.2 'creates a fresh ABI session' WRONG: helper calls cached nativeViewAbiSession() accessor. Fresh/full render path doesnot mean fresh runtime or cache isolation. Keep terminology precise (22/24 session cache).
2. Scout4.8 blanket 'state writes do not call renderApp again' too broad: narrow header and 1000siblings do not rerender, BUT keyed reorder AND keyed changed-label tests explicitly call consumer.renderApp() after items.set. Thus keyed reuse assertions do not independently prove automatic list-state scheduling. Direct source scoped-invalidation.test.ts ~132–146 confirms. Scope automaticclaim accordingly.
3. AppHarnessContract is local interface not exported; AppHarness/createAppHarness exported. Public/testing is supported subpath, not wholly private implementation even though intended tests.
4. Exact inventory counts inconsistent internally (projection filecount vs listed4; TS filecount roles; fixture 'two source/test files' vs source+2tests). Do not adopt aggregate counts as census. Detailed paths are navigation not verified quantitative inventory. Later census must compute exact classifications.

## Test evidence distinctions
Use source route+assertions, executionstatus, instrumentationlimits, oraclecoupling dimensions. Schema-generated agreement != independent ABI behavior; screen result != scopefrontier != total layout complexity != interactive timer proof. 25 is navigation; reconcile43 for actual contract strength and skipped/unreachable checks. No production edits or new tests.
