# PERF-13-G implementation notes

## Scope

PERF-13-G is the content-feature migration tranche. The plain F substrate is
now extended rather than bypassed:

```text
Source snapshot
    -> typed Funnel transform
    -> canonical semantic text IR
    -> Funnel delivery policy
    -> Connector-local projection/cache
    -> ContentHost layout/paint
```

The production path has no compatibility Funnel JSON descriptor and no native
`TextStream` renderer. `TextStream` is retained only as a typed compatibility
facade whose `update`, `append`, and `seal` operations terminate in the
Source direct-data ABI. Its History attachment sends scalar control values
(projector, delivery, pacing, and insets) through the control ABI.

## Content features

- `TextFunnel.plain`, `.markdown`, `.diff`, and `.ansi` are immutable,
  Source-neutral specifications.
- `.smooth(config)` is a Funnel delivery policy. Mutable smoothing state is
  created only for a demanded Connector and owns the native reveal clock,
  frontier, and delivery revision.
- Markdown, diff, and ANSI input all produce the generic text IR. ANSI SGR and
  OSC 8 display intent becomes semantic style/link data; unsafe terminal
  controls are consumed and never reach terminal output.
- Source annotations use the fixed v1 sidecar kinds (`tag`, `style`, `atomic`,
  and `point`). Style payloads contain semantic colors, optional theme roles,
  and typed text attributes; they contain no host-native style IDs.
- Source semantic roles/annotations are content revisions. Theme and
  `StyleRef` realization changes invalidate host presentation/projection
  caches without changing structure or reparsing semantic content.

## History and consumer seam

A native History stream attachment is now represented as an ordinary
ContentHost occurrence backed by a host-owned Port and Connector. History
height selection receives the same ContentProvider as body ContentHosts, so
content revisions participate in flow measurement and follow-end layout.

Detached native History retains pending Source-backed attachment descriptors
until host transfer; it does not create a second stream store. Once transferred,
the descriptors are materialized through the same Port/Connector path.

The old native `HostTextStream`/`NativeTextStream` production route was
removed. Existing generic Rust stream algorithms remain available only as
framework semantic algorithms and tests; they are not used by the native
TypeScript content path.

## Verification target

The final G gate requires the current consumer to use Source/Funnel/
Connector/ContentPort exclusively, with Markdown, diff, ANSI, smoothing,
annotations, History, and scrolling behavior preserved. PERF-12's full
benchmark suite remains intentionally deferred; use focused probes and the
relevant existing tests only.
