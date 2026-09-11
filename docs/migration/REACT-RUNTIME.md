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
  barrier. The runtime and native host own asynchronous frame, receipt, and
  source work; applications should not run a JavaScript frame pump.
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

`submittedUiRecords` measures commit records sent by the React coordinator.
`nativeCounters` measures actual native projection, layout, paint and content
work; it is not inferred from those submitted records. The result also records
delivered native output and the confirmed visible receipt revision.

## T5/M1 validation status

The old native View publication ABI, generated View outputs, and ordinary
Rust/native ViewState registry are absent from the current source. React is the
only production UI route. This is a deletion checkpoint, not M1 acceptance.
Animation ticking, persistent stop behavior, and pending-command/retirement
ordering corrections are deferred to a separate follow-up commit. The private
`LegacySceneAdapter`, current renderer/layout internals, native control
mechanics, and independent History/content helpers remain as M2 residue until
the direct Taffy and semantic-content deletion gates pass.

The deletion-only source passed `cargo fmt --all -- --check`,
`cargo check --workspace --all-features`, and `bun run check:ownership` on
macOS arm64. Earlier full-suite and packaged-addon results cover intermediate
working trees, not this exact checkpoint. The staged addon has not been rebuilt
for the deletion-only tree; final runtime validation follows the animation
corrections. Source content FFI remains part of the canonical addon, with
staging symbol checks and native `content_ffi::tests`; there is no separate
direct-FFI UI artifact or route. Linux x64 remains a CI-only matrix gate.
