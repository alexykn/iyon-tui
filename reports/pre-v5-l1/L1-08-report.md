# L1-08 completion report: Source FFI and persistent storage

Date: 2026-09-05. Base: `4fa8333` (L1-07 committed).

## What changed

**Zero-copy FFI annotation borrow (§9.2).**
- Declared `ContentAnnotationRecord` with `#[repr(C)]` and compile-time size (32 bytes) and alignment (4 bytes) assertions.
- Replaced the duplicate `IyonTuiAnnotationRecordV1` struct in `crates/iyon-tui-native/src/content_ffi.rs` with `pub type IyonTuiAnnotationRecordV1 = ContentAnnotationRecord;`.
- Deleted `copy_records` and the intermediate DTO vector allocation entirely. FFI mutations (`append_utf8`, `replace_utf8`) now borrow `&[ContentAnnotationRecord]` directly from the incoming N-API TypedArray without intermediate allocation or copy.

**Single ingestion scan (§9.3).**
- Introduced `ValidatedInput` in `crates/iyon-tui/src/application/source_store.rs`.
- Scans input byte slice once for UTF-8 validity and accumulates line ending offsets. Both `decode_annotations` and `retention_would_overflow` consume this single scan without re-scanning or re-validating the buffer.

**Persistent chunk tree storage (§9.4).**
- Implemented `ChunkTree` in `crates/iyon-tui/src/application/source_store.rs` with 16 KiB chunk descriptors (`Arc<ChunkNode>`).
- Supports logarithmic right-insert, absolute-offset split/slice for head truncation, and newline offset tracking.
- Replaced the legacy whole-deque/vector COW `SourceStorage` in `crates/iyon-tui/src/application/content.rs` with persistent `StoredSource`.
- Taking a snapshot (`HostContentSource::snapshot`) is an `Arc` refcount bump of `StoredSource`—zero payload bytes or chunks copied.

**Persistent annotation interval index (§9.5).**
- Implemented `AnnotationTree` in `crates/iyon-tui/src/application/source_store.rs` using an immutable balanced Treap ordered by `(start_byte, seqno)`.
- Augmented with subtree `max_end` coordinates for efficient interval overlap queries and subtree counts.
- Head truncation atomically drops out-of-bounds annotations and clips boundary intervals without linear scans or full metadata copies.

**Preflight arithmetic and atomic mutations (§9.6).**
- All mutations (`append_utf8`, `replace_utf8`, `clear`, `seal`, `truncate_head`) preflight fallible counters (`next_revision`, `content_generation`) and validate payload size/coordinates before swapping candidate storage roots.
- If a subscriber host mutex is poisoned, `finish_mutation` accepts the mutation, advances source revision, and returns `SOURCE_WAKE_FAILED` containing the accepted revision ID so callers do not duplicate mutations on retry.

## Stop-gate evidence

- **Persistent sharing & atomicity:** `stop_condition_persistent_sharing_across_repeated_appends`
  - Verifies 50 sequential appends with annotations maintain immutable previous snapshots without metadata copies.
  - Verifies atomic retention application prunes storage base and annotation intervals consistently.
- **Counter exhaustion preflight:**
  - `exhausted_source_revision_rejects_append_without_installing_anything`
  - `exhausted_source_revision_rejects_clear_without_touching_storage`
  - `exhausted_source_revision_rejects_replace_without_touching_storage`
  - `exhausted_source_revision_rejects_seal_without_setting_sealed`
  - `exhausted_source_revision_rejects_truncate_without_moving_head`
  - `exhausted_content_generation_rejects_clear_without_touching_storage`
- **Subscriber wake failure notification:**
  - `poisoned_subscriber_wake_reports_source_wake_failed_with_accepted_revision`
- **Treap invariants & storage semantics:**
  - 14 tests in `crates/iyon-tui/src/application/source_store.rs` covering single-chunk roundtrips, multi-chunk/page appends, newline edge boundaries, multibyte UTF-8 boundaries, treap property verification under random operations, stacked truncations, and sealed rejections.

## Verification results

- `cargo test --workspace`: 23 test suites pass, 763 passed, 0 failed, 2 ignored.
- `cargo test --features native-host,perf-counters --lib -p iyon-tui application::content`: 25 passed, 0 failed.
- `cargo test -p iyon-tui-native`: 56 passed, 0 failed.
- `bun run check:ownership`: ALL OWNERSHIP CHECKS PASSED.
- `bun test`: 113 passed, 0 failed across 32 files.
- `cargo clippy --workspace --all-targets`: 0 errors.
