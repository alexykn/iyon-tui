/**
 * PERF-13-E content-data ABI v1 constants. The fixed lane layout is shared
 * with crates/iyon-tui-native/src/content_ffi.rs; payload code never sends
 * JavaScript objects or JSON across this boundary.
 */
export const CONTENT_ABI_MAGIC = 0x494f_4e31;
export const CONTENT_ABI_VERSION = 1;
export const CONTENT_ABI_SEMANTIC_VERSION = 1;
export const CONTENT_ABI_METADATA_LANES = 32;
export const CONTENT_ABI_METADATA_BYTES = CONTENT_ABI_METADATA_LANES * 4;
export const CONTENT_ABI_ANNOTATION_LANES = 8;
export const CONTENT_ABI_ANNOTATION_BYTES = CONTENT_ABI_ANNOTATION_LANES * 4;
export const CONTENT_ABI_MUTATION_RESULT_LANES = 6;
export const CONTENT_ABI_MUTATION_RESULT_BYTES = CONTENT_ABI_MUTATION_RESULT_LANES * 4;
export const CONTENT_ABI_STATUS_VERSION = 1;
export const CONTENT_ABI_REQUIRED_SYMBOLS = 6;
export const CONTENT_ABI_ENDIAN_MARKER = 0x0102_0304;
export const CONTENT_ABI_SCHEDULE_ENVIRONMENT_DRAIN = 1;

export const CONTENT_ABI_BUILD_FINGERPRINT = [
  0x494f_4e53,
  0x362d_5445,
  0x5854_2d31,
  0,
  0,
  0,
  0,
  0,
] as const;
export const CONTENT_ABI_SCHEMA_FINGERPRINT = [
  0x5045_5246,
  0x3133_2d45,
  0x434f_4e54,
  0x454e_542d,
  0x4142_4931,
  0,
  0,
  0,
] as const;

export const CONTENT_STATUS = {
  OK: 0,
  INVALID_ARGUMENT: 1,
  ABI_MISMATCH: 2,
  WRONG_ENVIRONMENT: 3,
  STALE_ENVIRONMENT: 4,
  STALE_SOURCE: 5,
  SOURCE_DISPOSED: 6,
  SOURCE_IN_USE: 7,
  INVALID_UTF8: 8,
  INVALID_RANGE: 9,
  UNKNOWN_ANNOTATION_KIND: 10,
  INVALID_ANNOTATION_PAYLOAD: 11,
  LIMIT_EXCEEDED: 12,
  PAYLOAD_TOO_LARGE: 13,
  RUNTIME_POISONED: 14,
  INTERNAL_PANIC: 15,
  SOURCE_SEALED: 16,
  SOURCE_ALREADY_SEALED: 17,
  SOURCE_RETENTION_OVERFLOW: 18,
  INTERNAL_INVARIANT: 19,
} as const;

export type ContentStatusName = keyof typeof CONTENT_STATUS;
export type ContentStatusCode = (typeof CONTENT_STATUS)[ContentStatusName];

const statusNames: Readonly<Record<number, ContentStatusName>> = Object.fromEntries(
  Object.entries(CONTENT_STATUS).map(([name, code]) => [code, name as ContentStatusName]),
);

export function contentStatusName(status: number): ContentStatusName | undefined {
  return statusNames[status >>> 0];
}

export interface ContentAbiMetadata {
  readonly magic: number;
  readonly abiVersion: number;
  readonly semanticVersion: number;
  readonly pointerWidth: number;
  readonly endianMarker: number;
  readonly metadataSize: number;
  readonly mutationResultSize: number;
  readonly mutationResultAlign: number;
  readonly annotationRecordSize: number;
  readonly annotationRecordAlign: number;
  readonly annotationRecordLanes: number;
  readonly statusTableVersion: number;
  readonly requiredSymbolCount: number;
  readonly buildFingerprint: readonly number[];
  readonly schemaFingerprint: readonly number[];
}

export function decodeContentAbiMetadata(lanes: Uint32Array): ContentAbiMetadata {
  if (lanes.length < CONTENT_ABI_METADATA_LANES) {
    throw new Error("content ABI metadata buffer is too small");
  }
  return {
    magic: lanes[0]!,
    abiVersion: lanes[1]!,
    semanticVersion: lanes[2]!,
    pointerWidth: lanes[3]!,
    endianMarker: lanes[4]!,
    metadataSize: lanes[5]!,
    mutationResultSize: lanes[6]!,
    mutationResultAlign: lanes[7]!,
    annotationRecordSize: lanes[8]!,
    annotationRecordAlign: lanes[9]!,
    annotationRecordLanes: lanes[10]!,
    statusTableVersion: lanes[11]!,
    requiredSymbolCount: lanes[12]!,
    buildFingerprint: [...lanes.subarray(14, 22)],
    schemaFingerprint: [...lanes.subarray(22, 30)],
  };
}
