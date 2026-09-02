#ifndef IYON_CONTENT_ABI_H
#define IYON_CONTENT_ABI_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct IyonTuiPerf13AbiMetadataV1 {
    uint32_t magic;
    uint32_t abi_version;
    uint32_t semantic_version;
    uint32_t pointer_width;
    uint32_t endian_marker;
    uint32_t metadata_size;
    uint32_t mutation_result_size;
    uint32_t mutation_result_align;
    uint32_t annotation_record_size;
    uint32_t annotation_record_align;
    uint32_t annotation_record_lanes;
    uint32_t status_table_version;
    uint32_t required_symbol_count;
    uint32_t reserved0;
    uint32_t build_fingerprint[8];
    uint32_t schema_fingerprint[8];
    uint32_t reserved[2];
} IyonTuiPerf13AbiMetadataV1;

/*
 * kind=STYLE payloads use semantic-text-style-v1:
 * version:u8, flags:u8, attribute_presence:u8, attribute_values:u8,
 * followed by optional length-prefixed UTF-8 role/colors. The payload is
 * host-independent; it never contains a native Style ID.
 */
typedef struct IyonTuiAnnotationRecordV1 {
    uint32_t kind;
    uint32_t flags;
    uint32_t start_byte;
    uint32_t end_byte;
    uint32_t payload_offset;
    uint32_t payload_length;
    uint32_t aux0;
    uint32_t aux1;
} IyonTuiAnnotationRecordV1;

typedef struct IyonTuiSourceMutationResultV1 {
    uint32_t source_revision_lo;
    uint32_t source_revision_hi;
    uint32_t environment_wake_epoch_lo;
    uint32_t environment_wake_epoch_hi;
    uint32_t flags;
    uint32_t reserved0;
} IyonTuiSourceMutationResultV1;

enum {
    IYON_CONTENT_ANNOTATION_KIND_TAG = 1,
    IYON_CONTENT_ANNOTATION_KIND_STYLE = 2,
    IYON_CONTENT_ANNOTATION_KIND_ATOMIC = 3,
    IYON_CONTENT_ANNOTATION_KIND_POINT = 4,
};

enum {
    IYON_CONTENT_STATUS_OK = 0,
    IYON_CONTENT_STATUS_INVALID_ARGUMENT = 1,
    IYON_CONTENT_STATUS_ABI_MISMATCH = 2,
    IYON_CONTENT_STATUS_WRONG_ENVIRONMENT = 3,
    IYON_CONTENT_STATUS_STALE_ENVIRONMENT = 4,
    IYON_CONTENT_STATUS_STALE_SOURCE = 5,
    IYON_CONTENT_STATUS_SOURCE_DISPOSED = 6,
    IYON_CONTENT_STATUS_SOURCE_IN_USE = 7,
    IYON_CONTENT_STATUS_INVALID_UTF8 = 8,
    IYON_CONTENT_STATUS_INVALID_RANGE = 9,
    IYON_CONTENT_STATUS_UNKNOWN_ANNOTATION_KIND = 10,
    IYON_CONTENT_STATUS_INVALID_ANNOTATION_PAYLOAD = 11,
    IYON_CONTENT_STATUS_LIMIT_EXCEEDED = 12,
    IYON_CONTENT_STATUS_PAYLOAD_TOO_LARGE = 13,
    IYON_CONTENT_STATUS_RUNTIME_POISONED = 14,
    IYON_CONTENT_STATUS_INTERNAL_PANIC = 15,
    IYON_CONTENT_STATUS_SOURCE_SEALED = 16,
    IYON_CONTENT_STATUS_SOURCE_ALREADY_SEALED = 17,
    IYON_CONTENT_STATUS_SOURCE_RETENTION_OVERFLOW = 18,
    IYON_CONTENT_STATUS_INTERNAL_INVARIANT = 19,
    IYON_CONTENT_FLAG_SCHEDULE_ENVIRONMENT_DRAIN = 1,
};

uint32_t iyon_tui_perf13_abi_metadata_v1(
    IyonTuiPerf13AbiMetadataV1 *out,
    uint32_t out_size
);
uint32_t iyon_tui_source_append_utf8_v1(
    uint32_t environment_slot,
    uint32_t environment_generation,
    uint32_t source_slot,
    uint32_t source_generation,
    const uint8_t *bytes,
    uint32_t bytes_len,
    const IyonTuiAnnotationRecordV1 *annotations,
    uint32_t annotation_count,
    const uint8_t *annotation_payload,
    uint32_t annotation_payload_len,
    IyonTuiSourceMutationResultV1 *out
);
uint32_t iyon_tui_source_replace_utf8_v1(
    uint32_t environment_slot,
    uint32_t environment_generation,
    uint32_t source_slot,
    uint32_t source_generation,
    const uint8_t *bytes,
    uint32_t bytes_len,
    const IyonTuiAnnotationRecordV1 *annotations,
    uint32_t annotation_count,
    const uint8_t *annotation_payload,
    uint32_t annotation_payload_len,
    IyonTuiSourceMutationResultV1 *out
);
uint32_t iyon_tui_source_clear_v1(
    uint32_t environment_slot,
    uint32_t environment_generation,
    uint32_t source_slot,
    uint32_t source_generation,
    IyonTuiSourceMutationResultV1 *out
);
uint32_t iyon_tui_source_seal_v1(
    uint32_t environment_slot,
    uint32_t environment_generation,
    uint32_t source_slot,
    uint32_t source_generation,
    IyonTuiSourceMutationResultV1 *out
);
uint32_t iyon_tui_source_head_truncate_v1(
    uint32_t environment_slot,
    uint32_t environment_generation,
    uint32_t source_slot,
    uint32_t source_generation,
    uint32_t offset_lo,
    uint32_t offset_hi,
    IyonTuiSourceMutationResultV1 *out
);

#ifdef __cplusplus
}
#endif

#endif
