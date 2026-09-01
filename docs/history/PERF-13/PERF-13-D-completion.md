# PERF-13-D — Content identities, public nouns, and cold control

**Status:** implemented

## Delivered

- Environment-owned native Source registry with monotonic source identities,
  explicit text block/stream source kinds, membership accounting, and weak
  host/Connector subscription records.
- Host-owned ContentPort and Connector registries with distinct desired/visible
  mount state and requested/committed Connector state.
- Canonical TypeScript content nouns and factories:
  `TextStreamSource.create`, `TextBlockSource.create`, `TextFunnel.plain`,
  `tui.contentPort`, `port.connect(source, funnel)`, and
  `ContentConnector.activate/deactivate/dispose/status`.
- Backend-neutral `ContentPort` HandleId attachments and a structural
  `ContentHost` occurrence, including retained and cold structural lowering.
- H3 duplicate, stale, wrong-host, environment, family, and node-kind checks.
- Cold activation semantics: unmounted Connectors retain membership and the
  requested selection without projection, buffering, or Source wake
  subscriptions; mounted activation becomes visible only at frame commit.
- Native/unit synthetic activation-failure injection exercises the failed
  switch path: the prior visible Connector remains active while the requested
  Connector reports `failed`, and an explicit retry can later succeed.
- Transactional unmount, remount, switch, deactivation, and Connector
  disposal, with old visible state preserved until the host barrier commits.
- Explicit `SOURCE_IN_USE`, `PORT_IN_USE`, `PORT_MOUNTED`, and related
  lifecycle failures; host teardown forcibly releases host-owned content
  records while environment Sources survive.
- Existing Source payload mutation and projection remain intentionally outside
  this tranche and are reserved for PERF-13-E/F.

## Verification

Focused lifecycle, attachment, source-sharing, retained-composition, failed
switch, and ContentHost/ViewState smoke checks passed. The API-H3,
PERF-13-A/B, and native harness Bun tests passed (28 tests, 171 expectations).
Constrained Rust host/scene/layout/native/ABI-generator tests passed, along
with formatting, clippy, no-default-features checks, TypeScript/declaration
checks, ABI generation, ownership checks, native staging, and `git diff
--check`.
