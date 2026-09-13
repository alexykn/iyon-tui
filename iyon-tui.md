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
API. Its exact export set is checked by `bun run check:tui-binding`. Native
code must import core items through this module only.

The seam contains direct-occurrence contracts, generated schema values,
qualified resource/content types, host and control operations, passive
style/geometry/text values, and the performance counters required by the
native addon. It does not expose the fluent View DSL, `IntoView`, generic
renderer/projector extension surfaces, or application callbacks. The
re-exports point at their actual private runtime owners rather than at
crate-root compatibility aliases. Performance counters remain private to the
core and are exposed through `binding` only under the `perf-counters` feature
for the native measurement lane.

## Private runtime modules

The application kernel, controls, content and text IR, projection algebra,
source coordinates, history, scene/Taffy presentation, theme, and terminal
backends are crate-visible implementation modules. Their public items are
intentionally not reachable from the external crate root. Rust unit and
integration behavior tests that need those owners live under the relevant
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
  module other than `binding`.
- `bun run check:ownership` checks the TypeScript framework/package boundary,
  dependency direction, and the binding/mapping snapshots.
- `cargo test -p iyon-tui --lib` exercises the private runtime owners directly.
