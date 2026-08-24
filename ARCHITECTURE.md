# ARCHITECTURE — generic terminal UI framework

This repository is a generic terminal UI framework.
It MUST NOT contain agent, model, provider, prompt, tool-call, approval,
conversation, transcript, or Iyon application policy.
If a feature requires those concepts, it belongs in the Iyon application
repository (`alexykn/iyon`), which consumes this framework as an external
consumer after the repository separation completes.

## Layers

```text
TypeScript public facade        packages/iyon-runtime/src/tui/**   (becomes @iyon/tui in S4)
        |  private native contract seam (src/native.ts)
N-API / direct-FFI addon        crates/iyon-native (TUI modules; becomes iyon-tui-native in S3)
        |
Rust framework                  crates/iyon-tui
```

## Ownership rules

- `crates/iyon-tui` and the TUI-native modules must never depend on or import
  `iyon-core` or `iyon-api`. Enforced by `bun run check:ownership`.
- Framework TypeScript may import only framework modules plus the single
  native-contract seam (`packages/iyon-runtime/src/native.ts`). The
  `bun:ffi` transport lowering is transitional until S6 replaces it with
  generated safe N-API; do not build public API around transport details.
- Application code may consume the framework only through public surfaces:
  the `iyon:tui` alias or `@iyon/runtime/tui` (later `@iyon/tui`). Deep
  imports into `src/tui/**`, retained-DAG internals, View ABI internals,
  NodeId/NativeRef sidecars, or native addon implementation modules are
  forbidden and machine-checked.
- Themes, renderers, defaults, labels, and spinner/footer/composer policy are
  caller-owned. No product policy enters this framework behind generic names.

## Public API discipline

The TypeScript facade surface and the mapped Rust public surface are frozen in
snapshots checked by `bun run check:ownership`
(`tools/ownership/snapshots/iyon-tui-rust-surface.txt`,
`docs/repository-separation/s0/api-surface.json`). Adding or removing a public
export requires deliberately regenerating those snapshots in the same change;
application-specific names (`Agent`, `Provider`, `ToolExecution`, `Approval`,
`Transcript`, `KernelSession`, ...) are rejected outright. Public API parity
between Rust, native, and TypeScript layers is mandatory.

## Machine checks

Run before completing any change:

```sh
bun run check:ownership
```

The gates cover: Rust dependency direction, TUI-native module purity,
application-native module purity, framework Rust purity, framework TypeScript
import direction, application/runtime public-entrypoint-only imports, and both
public-surface snapshots.
