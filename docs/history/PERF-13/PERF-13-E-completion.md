# PERF-13-E — Retained Source data, direct ABI, and wake fan-out

**Status:** implemented

## Delivered

- Environment-owned retained UTF-8 Source storage with immutable 16 KiB
  chunks, absolute byte coordinates, content generations, revisions, line
  indexes, snapshots, retention, sealing, and head truncation.
- Atomic append/replace/clear/seal/truncate mutations with UTF-8 boundary and
  annotation validation, bounded payloads, accepted/copy/drop counters, and
  precise lifecycle statuses.
- Fixed-envelope annotation records for tag, style, atomic, and point ranges;
  annotation payloads remain separate from text bytes and are clipped/dropped
  according to kind during retention.
- Direct content-data ABI v1 metadata, status table, mutation result, C header,
  default-visible symbols, panic boundary, and same-artifact Bun `dlopen`
  loading. The `.node` used by Node-API and direct FFI is resolved once and
  retained for the JavaScript environment lifetime.
- TypeScript Source mutation, snapshot, statistics (including accepted-byte,
  copy, and retention-drop counters), identity, annotation, and retention APIs.
  Successful mutations return the native revision and wake
  diagnostics without flushing or invoking callbacks from the ABI call.
- Native Source subscriber marking remains host-independent. One environment
  wake-broker driver drains the native environment's complete affected-host
  set, so one Source mutation wakes all eligible hosts without a JavaScript
  subscription mirror or redundant per-host payload calls.

## V5 alignment without starting V5

PERF-13-E was kept compatible with the V5 direction without implementing the
V5 projection tranche. The deliberate adaptations are:

- Source identity, immutable snapshots, absolute UTF-8 coordinates, revisions,
  and semantic annotations are retained independently of any host, port, or
  viewport, so one Source can later feed width-specific Connector projections.
- Annotation payloads remain semantic and width-independent; terminal cells,
  backend glyphs, viewport geometry, and host-native styles do not enter Source
  storage.
- The direct ABI carries only semantic Source data and a scheduler hint. It does
  not encode projection/materialization policy, leaving Connector projection,
  viewport handling, and physical output for PERF-13-F.
- Existing stream snapshot/index concepts remain reusable implementation
  foundations rather than creating a second public mutation or scheduling
  architecture.

## Verification

Focused PERF-13-A/B/D tests, native Rust checks/clippy, direct metadata and
append/error probes, default artifact staging, symbol visibility, declaration
closure, ownership, package build, and `git diff --check` passed. The full
PERF-12 suite was not rerun.
