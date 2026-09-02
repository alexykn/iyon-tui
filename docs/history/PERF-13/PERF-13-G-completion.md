# PERF-13-G completion

Status: complete for the available framework and consumer fixtures.

## Delivered

- Re-read and applied the resolved three-plane handoff and v5 ownership
  boundary. G treats current content capabilities as migration targets rather
  than v5 deferrals.
- Added immutable text Funnel modes for plain text, Markdown, unified diff,
  and safe ANSI interpretation. All modes lower through the shared semantic
  text IR and host-time style resolution.
- Added Connector-local Smooth delivery. Smooth consumes source-grapheme
  delivery units after semantic transformation, keeps mutable clock/frontier
  state on the Connector, and schedules native frames without React or
  TypeScript per-tick traffic.
- Added fixed semantic Source style annotations with role/color/attribute
  descriptors. Source annotations remain environment/host independent and
  retention policies remain kind-specific.
- Passed the host Theme into content projection. Theme/StyleRef changes
  invalidate content presentation without structural publication or stale
  cached paint.
- Migrated native TypeScript `TextStream` storage and mutation to
  `TextStreamSource` plus the direct Source data ABI. `TextStream.update()` is
  a pure Source.replace adapter. History control uses typed scalar Funnel and
  delivery arguments; there is no compatibility Funnel JSON descriptor.
- Removed the native `NativeTextStream` and host `HostTextStream` production
  route. History-backed content is a ContentHost occurrence using the same
  Port/Connector provider as body content.
- Integrated History content measurement, follow-end selection, stable-prefix
  native row transfer for open plain streams, sealed-content transfer for all
  modes, and Connector teardown. Content transfer preserves ContentHost
  padding and releases the Connector only after row acceptance.
- Updated the available Iyon application fixture to create a
  `TextStreamSource`, explicit Markdown+Smooth Funnel, ContentPort, and
  Connector. Existing consumer streaming/Markdown/history tests pass against
  the staged G artifact.

## Verification

Passed:

- `cargo test -p iyon-tui --features native-host --lib` — 752 passed;
- native crate tests — 38 passed plus one ignored;
- Rust clippy with warnings denied;
- TypeScript typecheck and declaration closure;
- ownership/API/ABI checks;
- staged default and direct-FFI artifact probes;
- focused plain/Markdown/diff/ANSI/annotation/Smooth/History probes;
- available Iyon streaming, public application, and recovery tests.

The available Iyon public suite has one pre-existing dirty-worktree failure in
its effort-color expectation (`Yellow` versus the worktree's `LightYellow`);
no unrelated consumer styling change was overwritten.

The full PERF-12 benchmark suite was intentionally not run.
