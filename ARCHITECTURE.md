# ARCHITECTURE — generic terminal UI framework

This repository is a generic terminal UI framework.
It MUST NOT contain agent, model, provider, prompt, tool-call, approval,
conversation, transcript, or Iyon application policy.
If a feature requires those concepts, it belongs in the Iyon application
repository (`alexykn/iyon`), which consumes this framework as an external
consumer after the repository separation completes.

## Layers

```text
Public semantics                packages/iyon-tui/src/api/**   (`@iyon/tui`)
Retained semantic execution     packages/iyon-tui/src/composition/**
Live runtime/lifetime           packages/iyon-tui/src/runtime/**
Structural/native transport    packages/iyon-tui/src/transport/**
Testing facade                 packages/iyon-tui/src/testing/** (`@iyon/tui/testing`)
        |
generated safe N-API addon     crates/iyon-tui-native
        | direct-FFI symbols remain feature-gated for qualification/oracle/rollback
        |
Rust framework                 crates/iyon-tui
```

## H3 ownership boundary

The immutable semantic View model is the common language between composition
and transport:

```text
api/view/**                 immutable semantic View, NodeId, children, styles,
                            derivations, and semantic sequence facts
composition/**              semantic slot reuse, execution scopes, state
                            subscriptions, and prepared publication protocol
runtime/**                  lifecycle/orchestration and concrete target binding
transport/structural/**     ABI encoding, NativeRef/lease retention, generated
                            calls, native component resolution, and cold bridge
                            lowering
```

Composition may inspect semantic View identity and fields, but it must not
import structural/native transport. Structural transport may consume semantic
nodes, but it must not import composition implementation details. `PersistentSeq`
remains a composition-owned semantic retention optimization and is exposed to
transport only through the read-only `SemanticSequence` contract.

Structural publication is intentionally narrow:

```ts
StructuralPublicationTarget.preparePublication(view)
  -> PreparedStructuralPublication | undefined
PreparedStructuralPublication.commit() | abort()
```

Preparation is fallible and atomic; commit promotes prepared physical state;
abort leaves the previously committed frame authoritative. The contract carries
only semantic `View` values and does not include native references, ABI records,
leases, or future state/content transport.

## Ownership rules

- `crates/iyon-tui` and `crates/iyon-tui-native` must never depend on or import
  `iyon-core` or `iyon-api`. Enforced by `bun run check:ownership`.
- Framework TypeScript may import only framework modules. Raw native contracts
  and addon loading are private to `packages/iyon-tui/src/transport/native/`;
  structural ABI/schema/generated code is private to
  `packages/iyon-tui/src/transport/abi/structural/`. The default transport is
  generated safe N-API over opaque native session/host objects; the generated
  semantic DAG, leases, NativeRef hints, PersistentSeq edits, payload lanes,
  and stream specialization are transport-independent. Direct-FFI symbols
  remain feature-gated for qualification, oracle comparison, and rollback
  through later PERF/S tranches; they are not part of the default addon or
  public package contract.
- Application code in `alexykn/iyon` consumes the framework only through public
  surfaces: the `@iyon/tui` package or its application-owned `iyon:tui` alias.
  Deep imports into `packages/iyon-tui/src/**`, retained-DAG internals, View ABI
  internals, NodeId/NativeRef sidecars, or native addon implementation modules
  are forbidden. The application repository owns the corresponding consumer
  gate; this repository checks the public package and standalone fixture.
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
bun run check:tui-abi
bun run typecheck
bun run check:tui-declarations
bun run check:ownership
```

The gates cover: Rust dependency direction, TUI-native module purity,
framework Rust/TypeScript purity, composition/runtime/native ownership,
public declaration closure, package/publication boundaries, the standalone
consumer's public dependency, and both public-surface snapshots.
