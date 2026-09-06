# `iyon-tui` Rust runtime boundary

`crates/iyon-tui` is an unpublished implementation crate for the generic
TypeScript TUI framework and its in-tree native addon. It is not a supported
Rust UI-authoring package. Rust applications must not rely on its internal
semantic constructors, projectors, renderers, controls, scenes, history, or
coordinate modules.

## Public modules

### `iyon_tui::binding`

`binding` is the one deliberately unsupported cross-crate seam. It exists so
`crates/iyon-tui-native` can link to the runtime; it is not a Rust authoring
API. Its exact export set is checked by `bun run check:tui-binding` (currently
124 operation and passive-type exports). Native code must import core items
through this module only.

The seam contains operation-specific retained-view constructors, typed state
and content records, host operations, passive style/geometry/text values, and
measurement helpers required by the native addon. It does not expose the
fluent View DSL, `IntoView`, generic renderer/projector extension surface, or
application callbacks. The re-exports point at their actual private runtime
owners rather than at crate-root compatibility aliases.

### `iyon_tui::perf_bench` (feature `perf-counters`)

This `#[doc(hidden)]` module is an executable-only exception retained for the
package-local `tui_perf` benchmark target. It is feature-gated and is not a
supported runtime or authoring surface. Performance counters themselves are
private to the core and are re-exported to the native measurement lane only
through `binding`.

## Private runtime modules

The application kernel, controls, content and text IR, projection algebra,
source coordinates, history, scene, theme, retained state, presentation API,
and terminal backends are crate-visible implementation modules. Their public
items are intentionally not reachable from the external crate root. Rust unit
and integration behavior tests that need those owners live under the relevant
`src/**` module tests; the former external authoring-test facade and its
`trybuild` root contract are removed.

The `test-util` Cargo feature remains only for in-tree/native test hooks and
instrumentation. It does not restore a public Rust `testing` module. The
TypeScript testing contract remains available at `@iyon/tui/testing` and is
independent of this Rust-only boundary.

## Enforcement

- `publish = false` prevents the core crate from being treated as a published
  authoring package.
- `bun run check:tui-binding` rejects root `pub use` leaks and any public root
  module other than `binding` and the hidden benchmark exception.
- `bun run check:ownership` checks the TypeScript framework/package boundary,
  dependency direction, and the binding/mapping snapshots.
- `cargo test -p iyon-tui --lib` exercises the private runtime owners directly.
