# PERF-11v4 archaeology record

Recorded on `perf-refactor` before adding the benchmark-only Candidate A reconstruction.

## Revisions and environment

```text
current PERF-11v3 HEAD: e8533d61014eedc0d505fe1bbca8e1883746ce95
historical PERF-7v2 Candidate A: e5292d62c4011610850cbdc1ba4a35f296f78e4f
Bun --version: 1.4.0
Bun --revision: 1.4.0+34cbb9a40
rustc: rustc 1.97.1 (8bab26f4f 2026-07-14)
target: aarch64-apple-darwin
macOS: 26.5.2
CPU model: MacBookPro18,3
```

## Current direct-path audit

```text
A. N-API Object decoder exists: yes
   crates/iyon-native/src/tui.rs:1811 (`decode_view`) and `ViewDecoder`.

B. NativeTuiHost.render(Object) enters it: yes
   crates/iyon-native/src/tui.rs:868-871.

C. It reads NodeId before recursive decoding: yes
   `ViewDecoder::decode` reads `id`, validates the header, then decodes misses.

D. A live NodeId -> WeakView hit stops traversal: yes
   `cache.nodes.get(&node_id).and_then(WeakView::upgrade)` returns immediately.

E. The semantic cache is the current environment runtime cache: yes
   direct decoding calls `view_bridge_cache`; current NativeViewRuntime owns the
   same semantic NodeId/WeakView state used by the retained/generated routes.

F. Expired WeakView cleanup is present: yes
   stale entries are removed before cache-miss decoding.

G. The complete current BridgeViewNode schema is supported: yes
   text, diff, spacer, row, column, hanging, grid, container, clamp,
   content-max, component, and decorated nodes are handled.

Normal `bun run build:iyon` compiles the direct path: yes.
Normal-build execution proof: passed. A staged addon rendered a valid bridge
node through `NativeTuiHost.render(Object)`, and an altered `schema` failed in
the Direct decoder with `unsupported TUI View bridge schema 999`.
```

## Historical/current representation comparison

The historical source was extracted directly with:

```text
git show e5292d62c4011610850cbdc1ba4a35f296f78e4f:packages/iyon-runtime/src/tui/values/view.ts
```

Its constructor immediately stores `withPrivateIdentity(node)` in a
`WeakMap<View, BridgeViewNode>` and freezes the public `View`. `nodeForBridge`
is therefore a WeakMap lookup. Current `View` instead stores stable backing
states (`materialized`, `pending create`, and `pending patch`) and
`nodeForBridge` may materialize the rich bridge node lazily.

```text
historical JS eager DAG unchanged: no
current direct compatibility candidate available: yes
faithful Candidate A benchmark reconstruction possible in current checkout: yes
second complete checkout required: no
```

The benchmark-only reconstruction must preserve the historical eager immutable
DAG and adapt only imports/schema typing required by the current correct schema.
It must not mutate production `View`, the native decoder, the cache, renderer,
or host APIs.
