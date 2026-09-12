# `iyon-tui`

`iyon-tui` is the unpublished Rust runtime implementation for the generic
terminal UI framework and its in-tree native addon. It owns terminal sessions,
input routing, clocks, retained presentation, layout, Unicode-safe painting,
History mechanics, content execution, and native control interaction.

Rust code in this crate is not a supported UI-authoring package. Application
authors use the TypeScript package `@iyon/tui` (and its `@iyon/tui/testing`
entry point), which preserves the public semantic control, content,
projection, theme, and testing contracts.

## Native binding boundary

The only cross-crate Rust seam is the deliberately unsupported
`iyon_tui::binding` module. It exists for `crates/iyon-tui-native`, not for
application code. The seam exposes only typed state/content ingress, host
operations, passive style and
geometry values, and the measurement functions required by the native addon.
Native code must import core items through `iyon_tui::binding`.

The binding does not expose a fluent semantic UI DSL, generic renderer or
projector extension APIs, arbitrary callbacks into the hot path, or an
application-specific policy. Its export set is pinned by
`bun run check:tui-binding`; native imports and root visibility are checked
before integration.

## Internal organization

Semantic text IR, projectors, source coordinates, occurrence state, History,
controls, themes, and direct presentation APIs are crate-visible runtime
modules. Their public item declarations support the binding and the in-crate
unit tests but are not reachable through the external crate root. Built-in
runtime code lowers occurrences directly through Taffy and physical products.

The former Rust integration-test authoring facade and `trybuild` root export
contract are intentionally removed. Behavioral tests that need private owners
live under the relevant `src/**` module tests. The `test-util` Cargo feature is
kept only for in-tree/native test hooks and instrumentation; it does not
restore a public Rust `testing` module.

## Tooling

- `bun run check:tui-binding` checks the native seam, its allowlist, and the
  binding-only Rust root boundary.
- `bun run check:ownership` checks package/public TypeScript ownership,
  dependency direction, and the committed mapping snapshots.
- `cargo test -p iyon-tui --lib` exercises the private runtime owners.
- `cargo test -p iyon-tui-native` exercises the native adapters.
- `cargo fmt --all -- --check` verifies Rust formatting.

See the repository's [AGENTS.md](../../AGENTS.md) for ownership rules and the
[archived Rust lowering handoff](../../docs/history/PRE-V5/PRE-V5-RUST-LOWERING-HANDOFF.md)
for historical migration rationale.
