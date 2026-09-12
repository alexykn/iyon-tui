# React runtime migration

The supported TypeScript UI route is now the React host API. A consumer opens
one terminal runtime, creates one React root, and renders React components into
that root:

```ts
import { createElement } from "react";
import { Tui } from "@iyon/tui";
import { Text, createReactRoot } from "@iyon/tui/react";

const tui = await Tui.open();
const root = createReactRoot(tui);
await root.render(createElement(Text, {}, "hello"));
await root.whenVisible();
```

## API changes

- Replace `Tui.render(Scene)` and handwritten `View`/composition code with
  `createReactRoot(tui)` and `root.render(children)`.
- Create caller-owned content resources only after the root exists:
  `const port = tui.contentPort()`, then `port.connect(source, funnel)`. Pass
  the resulting `ContentPort` or `ContentConnector` to the React `Content`
  component. These resources survive unmount and must be detached or
  deactivated before `dispose()`.
- Use `root.whenVisible()` or `root.whenContentVisible()` as the presentation
  barrier. `root.render()` acknowledges desired acceptance, not painting.
  `whenVisible()` permits loading or retained content; use
  `whenContentVisible()` when the desired text/product must be physically
  visible. Neither React-revision barrier waits for a later native key or
  animation tick, because those do not change the React revision. The runtime
  and native host own asynchronous frame, receipt, and Source work;
  applications should not run a JavaScript frame pump.
- Consume terminal output and termination through `tui.nextEvent(signal)`.
  `Tui.bindKey()` retains global routing, while an accepted `Editor` ref can
  call `focus()` and `interceptPaste(routeId)` for targeted paste forwarding.
- Use `History` and `HistoryUnit` React components for physical History
  transfer. A History transfer must obey the existing unknown-suffix barrier;
  uncertain suffixes are not replayed.

## Lifecycle

There is one live React root per `Tui` host. Close the root before closing the
runtime when the application needs explicit teardown; `tui.close()` and
`tui.exit()` also invalidate the root authority. A runtime cannot create a
root after it has closed. `onRuntimeError` receives structured native
projection, scheduler, and presentation failures without requiring a barrier
or status poll.

The `@iyon/tui/testing` `AppHarness` implements the same root, output, content,
and lifecycle contract as `Tui` while retaining deterministic `advance()` and
snapshot helpers for tests.

## Native content benchmark

The React benchmark uses the existing opt-in Rust performance counters. Build
the instrumented addon explicitly, then run the benchmark:

```sh
ION_NATIVE_FEATURES=perf-counters bun run native:stage
bun run perf:content
bun run native:stage # restore the ordinary addon for development and checks
```

`trafficWitnesses` and per-sample `traffic` measure actual coordinator calls,
records, and separately accounted word/metadata/content bytes. Source accepted
and copied bytes come from its native stats. `nativeCounters` measures native
work, not classifications of submitted records. The instrumented build times
worker-owned projection, Taffy layout, shared physical paint, and host frame
stages. Timers compile out of the default build.

Set `ION_PERF_OUTPUT` to retain JSON; `ION_PERF_SAMPLES` defaults to seven and
`ION_PERF_WARMUPS` to one. Seven-sample p99 values are exploratory. Native-control
benchmark barriers observe physical host epochs and changed editor/animation
frames, with benchmark-side timer/inspection cost included. Do not interpret
these end-to-end samples as isolated native execution time.

## Current runtime and validation

React is the only production UI route. The old View ABI, ViewState registry,
immutable publication path, `LegacySceneAdapter`, general View layout allocator,
and TextRenderer lowering are deleted. Occurrences feed Taffy directly;
semantic content uses the terminal content projector. Native controls and
independent physical History behavior remain intentional owners, not a second
UI publication route. Source content FFI is part of the canonical addon; there
is no separate direct-FFI UI artifact.

Projection admission is bounded and asynchronous. Saturation defers metadata
and wakes the owner when capacity returns; it does not block React acceptance
or create an unbounded snapshot queue. Layout and paint run on their worker,
and receipt/close waits do not hold the host or global environment drain lock.

Current checks, source/addon hashes, raw benchmark locations, and remaining
comparison limitations are recorded in
[`DOM-RUNTIME-IMPLEMENTATION.md`](../architecture/DOM-RUNTIME-IMPLEMENTATION.md).
Linux execution and GPUI implementation are outside this assignment. Archived
source/addon pairs are diagnostic artifacts; a schema-mismatched addon must
never be loaded into the current benchmark to claim a baseline comparison.
