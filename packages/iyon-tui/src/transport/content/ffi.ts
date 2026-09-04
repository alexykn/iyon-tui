import { dlopen } from "bun:ffi";

import { TuiError } from "../../api/errors.ts";
import type { TextSourceAnnotation, SemanticTextStyle } from "../../api/content/retained.ts";
import type { TextAttribute } from "../../api/presentation/style.ts";
import type { ColorSpec } from "../../api/presentation/theme.ts";
import { nativeArtifact, type NativeTextSourceContract } from "../native/addon.ts";
import {
  CONTENT_ABI_ANNOTATION_LANES,
  CONTENT_ABI_ANNOTATION_BYTES,
  CONTENT_ABI_BUILD_FINGERPRINT,
  CONTENT_ABI_ENDIAN_MARKER,
  CONTENT_ABI_MAGIC,
  CONTENT_ABI_METADATA_BYTES,
  CONTENT_ABI_METADATA_LANES,
  CONTENT_ABI_MUTATION_RESULT_BYTES,
  CONTENT_ABI_MUTATION_RESULT_LANES,
  CONTENT_ABI_REQUIRED_SYMBOLS,
  CONTENT_ABI_SCHEMA_FINGERPRINT,
  CONTENT_ABI_SEMANTIC_VERSION,
  CONTENT_ABI_STATUS_VERSION,
  CONTENT_ABI_VERSION,
  CONTENT_ABI_SCHEDULE_ENVIRONMENT_DRAIN,
  contentStatusName,
} from "./abi.ts";
import { runtimeEnvironment } from "../../runtime/environment.ts";

const MAX_U32 = 0xffff_ffff;
const MAX_U64 = 0xffff_ffff_ffff_ffffn;
const MAX_SOURCE_PAYLOAD_BYTES = 64 * 1024 * 1024;
const MAX_ANNOTATION_PAYLOAD_BYTES = 4 * 1024 * 1024;
const encoder = new TextEncoder();

const annotationKinds = {
  tag: 1,
  style: 2,
  atomic: 3,
  point: 4,
} as const;
type AnnotationKind = keyof typeof annotationKinds;
const STYLE_PAYLOAD_VERSION = 1;
const STYLE_FLAG_ROLE = 1 << 0;
const STYLE_FLAG_FOREGROUND = 1 << 1;
const STYLE_FLAG_BACKGROUND = 1 << 2;
const STYLE_FLAG_ATTRIBUTES = 1 << 3;
const STYLE_ATTRIBUTE_BITS: Readonly<Record<TextAttribute, number>> = {
  bold: 1,
  dim: 2,
  italic: 4,
  underline: 8,
  reversed: 16,
  strikethrough: 32,
};
const ANSI_COLOR_VALUES = [
  "black", "red", "green", "yellow", "blue", "magenta", "cyan", "gray",
  "darkGray", "lightRed", "lightGreen", "lightYellow", "lightBlue", "lightMagenta",
  "lightCyan", "white",
] as const;

interface EncodedAnnotations {
  readonly records: Uint32Array;
  readonly payload: Uint8Array;
}

interface MutationResult {
  readonly source_revision_lo: number;
  readonly source_revision_hi: number;
  readonly environment_wake_epoch_lo: number;
  readonly environment_wake_epoch_hi: number;
  readonly flags: number;
  readonly reserved0: number;
}

interface ContentFfiSymbols {
  readonly iyon_tui_perf13_abi_metadata_v1: (out: Uint8Array | Uint32Array, outSize: number) => number;
  readonly iyon_tui_source_append_utf8_v1: (
    environmentSlot: number,
    environmentGeneration: number,
    sourceSlot: number,
    sourceGeneration: number,
    bytes: Uint8Array,
    bytesLength: number,
    records: Uint32Array,
    annotationCount: number,
    annotationPayload: Uint8Array,
    annotationPayloadLength: number,
    out: Uint32Array,
  ) => number;
  readonly iyon_tui_source_replace_utf8_v1: (
    environmentSlot: number,
    environmentGeneration: number,
    sourceSlot: number,
    sourceGeneration: number,
    bytes: Uint8Array,
    bytesLength: number,
    records: Uint32Array,
    annotationCount: number,
    annotationPayload: Uint8Array,
    annotationPayloadLength: number,
    out: Uint32Array,
  ) => number;
  readonly iyon_tui_source_clear_v1: (
    environmentSlot: number,
    environmentGeneration: number,
    sourceSlot: number,
    sourceGeneration: number,
    out: Uint32Array,
  ) => number;
  readonly iyon_tui_source_seal_v1: (
    environmentSlot: number,
    environmentGeneration: number,
    sourceSlot: number,
    sourceGeneration: number,
    out: Uint32Array,
  ) => number;
  readonly iyon_tui_source_head_truncate_v1: (
    environmentSlot: number,
    environmentGeneration: number,
    sourceSlot: number,
    sourceGeneration: number,
    offsetLow: number,
    offsetHigh: number,
    out: Uint32Array,
  ) => number;
}

interface ContentFfiSession {
  /** Keeps Bun's dlopen handle alive for the whole JS environment. */
  readonly library: object;
  readonly symbols: ContentFfiSymbols;
  readonly metadata: Uint32Array;
  readonly artifactPath: string;
}

const sessions = new WeakMap<object, ContentFfiSession>();
const sourceIdentities = new WeakMap<NativeTextSourceContract, [number, number, number, number]>();

function contentError(
  code: string,
  message: string,
  context?: Readonly<Record<string, unknown>>,
): TuiError {
  const category = code === "ABI_MISMATCH"
    || code === "RUNTIME_POISONED"
    || code === "INTERNAL_PANIC"
    || code === "INTERNAL_INVARIANT"
    ? "runtime"
    : "validation";
  return new TuiError(category, `${code}: ${message}`, `ION_${code}`, context);
}

function assertU32(value: number, name: string): void {
  if (!Number.isSafeInteger(value) || value < 0 || value > MAX_U32) {
    throw contentError("INVALID_ARGUMENT", `${name} must fit in an unsigned 32-bit lane`);
  }
}

function splitU64(value: bigint | number, name: string): [number, number] {
  if (typeof value !== "bigint" && typeof value !== "number") {
    throw contentError("INVALID_ARGUMENT", `${name} must be a safe integer or bigint`);
  }
  if (typeof value === "number" && !Number.isSafeInteger(value)) {
    throw contentError("INVALID_ARGUMENT", `${name} must be a safe integer or bigint`);
  }
  const normalized = typeof value === "bigint" ? value : BigInt(value);
  if (normalized < 0n || normalized > MAX_U64) {
    throw contentError("INVALID_ARGUMENT", `${name} must fit in an unsigned 64-bit lane`);
  }
  return [Number(normalized & 0xffff_ffffn), Number(normalized >> 32n)];
}

function joinU64(low: number, high: number): bigint {
  return BigInt(low >>> 0) | (BigInt(high >>> 0) << 32n);
}

function mutationResult(result: Uint32Array): MutationResult {
  if (result.length < CONTENT_ABI_MUTATION_RESULT_LANES) {
    throw contentError("ABI_MISMATCH", "content mutation result buffer is too small");
  }
  return {
    source_revision_lo: result[0]!,
    source_revision_hi: result[1]!,
    environment_wake_epoch_lo: result[2]!,
    environment_wake_epoch_hi: result[3]!,
    flags: result[4]!,
    reserved0: result[5]!,
  };
}

function finishMutation(
  status: number,
  result: Uint32Array,
  requestWake: () => void,
): { readonly revision: bigint; readonly environmentWakeEpoch: bigint; readonly scheduleEnvironmentDrain: boolean } {
  const name = contentStatusName(status);
  if (name === undefined) {
    throw contentError("ABI_MISMATCH", `unknown content ABI status ${status >>> 0}`);
  }
  if (name !== "OK") {
    const message = name === "SOURCE_SEALED"
      ? "source is sealed"
      : `content Source mutation failed with ${name}`;
    throw contentError(name, message);
  }
  const decoded = mutationResult(result);
  if (decoded.reserved0 !== 0 || (decoded.flags & ~CONTENT_ABI_SCHEDULE_ENVIRONMENT_DRAIN) !== 0) {
    throw contentError("ABI_MISMATCH", "content mutation result contains reserved bits");
  }
  if ((decoded.flags & CONTENT_ABI_SCHEDULE_ENVIRONMENT_DRAIN) !== 0) requestWake();
  return {
    revision: joinU64(decoded.source_revision_lo, decoded.source_revision_hi),
    environmentWakeEpoch: joinU64(
      decoded.environment_wake_epoch_lo,
      decoded.environment_wake_epoch_hi,
    ),
    scheduleEnvironmentDrain: (decoded.flags & CONTENT_ABI_SCHEDULE_ENVIRONMENT_DRAIN) !== 0,
  };
}

function validateMetadata(metadata: Uint32Array): void {
  const mismatches: string[] = [];
  const expect = (actual: number, expected: number, name: string): void => {
    if (actual !== expected) mismatches.push(`${name}=${actual}, expected ${expected}`);
  };
  expect(metadata[0]!, CONTENT_ABI_MAGIC, "magic");
  expect(metadata[1]!, CONTENT_ABI_VERSION, "abiVersion");
  expect(metadata[2]!, CONTENT_ABI_SEMANTIC_VERSION, "semanticVersion");
  expect(metadata[3]!, process.arch === "ia32" ? 4 : 8, "pointerWidth");
  expect(metadata[4]!, CONTENT_ABI_ENDIAN_MARKER, "endianMarker");
  expect(metadata[5]!, CONTENT_ABI_METADATA_BYTES, "metadataSize");
  expect(metadata[6]!, CONTENT_ABI_MUTATION_RESULT_BYTES, "mutationResultSize");
  expect(metadata[7]!, 4, "mutationResultAlign");
  expect(metadata[8]!, CONTENT_ABI_ANNOTATION_BYTES, "annotationRecordSize");
  expect(metadata[9]!, 4, "annotationRecordAlign");
  expect(metadata[10]!, CONTENT_ABI_ANNOTATION_LANES, "annotationRecordLanes");
  expect(metadata[11]!, CONTENT_ABI_STATUS_VERSION, "statusTableVersion");
  expect(metadata[12]!, CONTENT_ABI_REQUIRED_SYMBOLS, "requiredSymbolCount");
  expect(metadata[13]!, 0, "reserved0");
  for (let index = 0; index < CONTENT_ABI_BUILD_FINGERPRINT.length; index += 1) {
    expect(metadata[14 + index]!, CONTENT_ABI_BUILD_FINGERPRINT[index]!, `buildFingerprint[${index}]`);
    expect(metadata[22 + index]!, CONTENT_ABI_SCHEMA_FINGERPRINT[index]!, `schemaFingerprint[${index}]`);
  }
  expect(metadata[30]!, 0, "reserved[0]");
  expect(metadata[31]!, 0, "reserved[1]");
  if (mismatches.length > 0) {
    throw contentError("ABI_MISMATCH", `content ABI metadata mismatch: ${mismatches.join("; ")}`);
  }
}

function openSession(): ContentFfiSession {
  const artifact = nativeArtifact;
  const library = dlopen(artifact.absolutePath, {
    iyon_tui_perf13_abi_metadata_v1: { args: ["buffer", "u32"], returns: "u32" },
    iyon_tui_source_append_utf8_v1: {
      args: ["u32", "u32", "u32", "u32", "buffer", "u32", "buffer", "u32", "buffer", "u32", "buffer"],
      returns: "u32",
    },
    iyon_tui_source_replace_utf8_v1: {
      args: ["u32", "u32", "u32", "u32", "buffer", "u32", "buffer", "u32", "buffer", "u32", "buffer"],
      returns: "u32",
    },
    iyon_tui_source_clear_v1: { args: ["u32", "u32", "u32", "u32", "buffer"], returns: "u32" },
    iyon_tui_source_seal_v1: { args: ["u32", "u32", "u32", "u32", "buffer"], returns: "u32" },
    iyon_tui_source_head_truncate_v1: { args: ["u32", "u32", "u32", "u32", "u32", "u32", "buffer"], returns: "u32" },
  });
  const metadata = new Uint32Array(CONTENT_ABI_METADATA_LANES);
  const status = library.symbols.iyon_tui_perf13_abi_metadata_v1(metadata, metadata.byteLength);
  const name = contentStatusName(status);
  if (name !== "OK") {
    throw contentError(name ?? "ABI_MISMATCH", `content ABI metadata probe failed with ${status >>> 0}`);
  }
  validateMetadata(metadata);
  return {
    library,
    symbols: library.symbols as ContentFfiSymbols,
    metadata,
    artifactPath: artifact.absolutePath,
  };
}

function session(): ContentFfiSession {
  const environment = runtimeEnvironment().resources.environment;
  const existing = sessions.get(environment);
  if (existing !== undefined) return existing;
  const created = openSession();
  sessions.set(environment, created);
  return created;
}

function sourceIdentity(source: NativeTextSourceContract): [number, number, number, number] {
  const existing = sourceIdentities.get(source);
  if (existing !== undefined) return existing;
  const environmentSlot = source.environmentSlot();
  const environmentGeneration = source.environmentGeneration();
  const sourceSlot = source.sourceId();
  const sourceGeneration = source.sourceGeneration();
  assertU32(environmentSlot, "environment slot");
  assertU32(environmentGeneration, "environment generation");
  assertU32(sourceSlot, "Source slot");
  assertU32(sourceGeneration, "Source generation");
  const identity: [number, number, number, number] = [
    environmentSlot,
    environmentGeneration,
    sourceSlot,
    sourceGeneration,
  ];
  sourceIdentities.set(source, identity);
  return identity;
}

function validateAnnotationObject(annotation: TextSourceAnnotation): void {
  if (typeof annotation !== "object" || annotation === null) {
    throw contentError("INVALID_ANNOTATION_PAYLOAD", "Source annotation must be an object");
  }
  for (const key of Object.keys(annotation)) {
    if (!["kind", "startByte", "endByte", "namespace", "name", "style", "payload"].includes(key)) {
      throw contentError("INVALID_ANNOTATION_PAYLOAD", `unknown Source annotation field ${JSON.stringify(key)}`);
    }
  }
}

function utf8ByteLength(text: string, maximum: number, message: string): number {
  let length = 0;
  for (let index = 0; index < text.length; index += 1) {
    const code = text.charCodeAt(index);
    let increment: number;
    if (code <= 0x7f) {
      increment = 1;
    } else if (code <= 0x7ff) {
      increment = 2;
    } else if (code >= 0xd800 && code <= 0xdbff
      && index + 1 < text.length
      && text.charCodeAt(index + 1) >= 0xdc00
      && text.charCodeAt(index + 1) <= 0xdfff) {
      increment = 4;
      index += 1;
    } else {
      // TextEncoder encodes an unpaired surrogate as U+FFFD (three bytes).
      increment = 3;
    }
    if (length > maximum - increment) {
      throw contentError("PAYLOAD_TOO_LARGE", message);
    }
    length += increment;
  }
  return length;
}

function annotationPayload(
  annotation: TextSourceAnnotation,
  kind: AnnotationKind,
  maximum: number,
): Uint8Array {
  if (kind === "style") {
    if (annotation.payload !== undefined || annotation.namespace !== undefined || annotation.name !== undefined) {
      throw contentError("INVALID_ANNOTATION_PAYLOAD", "style annotations use a semantic style, not tag names or opaque payload");
    }
    if (annotation.style === undefined) {
      throw contentError("INVALID_ANNOTATION_PAYLOAD", "style annotations require a semantic style");
    }
    return encodeSemanticStyle(annotation.style, maximum);
  }
  if (annotation.style !== undefined) {
    throw contentError("INVALID_ANNOTATION_PAYLOAD", `${kind} annotations do not accept a semantic style`);
  }
  if (kind === "tag") {
    if (annotation.payload !== undefined) {
      throw contentError("INVALID_ANNOTATION_PAYLOAD", "tag annotations use namespace and name, not payload");
    }
    if (typeof annotation.namespace !== "string" || typeof annotation.name !== "string") {
      throw contentError("INVALID_ANNOTATION_PAYLOAD", "tag annotations require namespace and name");
    }
    if (annotation.namespace.includes("\0") || annotation.name.includes("\0")) {
      throw contentError("INVALID_ANNOTATION_PAYLOAD", "tag annotation names must not contain NUL");
    }
    const namespaceLength = utf8ByteLength(
      annotation.namespace,
      maximum,
      "Source annotation payload is too large",
    );
    if (namespaceLength >= maximum) {
      throw contentError("PAYLOAD_TOO_LARGE", "Source annotation payload is too large");
    }
    const nameLength = utf8ByteLength(
      annotation.name,
      maximum - namespaceLength - 1,
      "Source annotation payload is too large",
    );
    const namespace = encoder.encode(annotation.namespace);
    const name = encoder.encode(annotation.name);
    const payload = new Uint8Array(namespaceLength + 1 + nameLength);
    payload.set(namespace, 0);
    payload[namespaceLength] = 0;
    payload.set(name, namespaceLength + 1);
    return payload;
  }
  if (annotation.namespace !== undefined || annotation.name !== undefined) {
    throw contentError("INVALID_ANNOTATION_PAYLOAD", `${kind} annotations do not accept tag names`);
  }
  if (annotation.payload === undefined) return new Uint8Array(0);
  if (!(annotation.payload instanceof Uint8Array)) {
    throw contentError("INVALID_ANNOTATION_PAYLOAD", "annotation payload must be a Uint8Array");
  }
  if (annotation.payload.byteLength > maximum) {
    throw contentError("PAYLOAD_TOO_LARGE", "Source annotation payload is too large");
  }
  return annotation.payload;
}

function encodeSemanticStyle(style: SemanticTextStyle, maximum: number): Uint8Array {
  if (typeof style !== "object" || style === null) {
    throw contentError("INVALID_ANNOTATION_PAYLOAD", "semantic style must be an object");
  }
  for (const key of Object.keys(style)) {
    if (key !== "role" && key !== "foreground" && key !== "background" && key !== "attributes") {
      throw contentError("INVALID_ANNOTATION_PAYLOAD", `unknown semantic style field ${JSON.stringify(key)}`);
    }
  }
  const chunks: number[] = [STYLE_PAYLOAD_VERSION, 0, 0, 0];
  let flags = 0;
  if (style.role !== undefined) {
    validateStyleName(style.role, "style role");
    flags |= STYLE_FLAG_ROLE;
    appendStyleString(chunks, style.role, maximum);
  }
  if (style.foreground !== undefined) {
    flags |= STYLE_FLAG_FOREGROUND;
    appendStyleColor(chunks, style.foreground, maximum);
  }
  if (style.background !== undefined) {
    flags |= STYLE_FLAG_BACKGROUND;
    appendStyleColor(chunks, style.background, maximum);
  }
  const attributes = style.attributes;
  if (attributes !== undefined) {
    if (typeof attributes !== "object" || attributes === null) {
      throw contentError("INVALID_ANNOTATION_PAYLOAD", "semantic text attributes must be an object");
    }
    flags |= STYLE_FLAG_ATTRIBUTES;
    for (const key of Object.keys(attributes)) {
      if (!(key in STYLE_ATTRIBUTE_BITS)) {
        throw contentError("INVALID_ANNOTATION_PAYLOAD", `unknown semantic text attribute ${JSON.stringify(key)}`);
      }
      const value = attributes[key as TextAttribute];
      if (typeof value !== "boolean") {
        throw contentError("INVALID_ANNOTATION_PAYLOAD", `semantic text attribute ${JSON.stringify(key)} must be boolean`);
      }
      chunks[2] = (chunks[2]! | STYLE_ATTRIBUTE_BITS[key as TextAttribute]!) & 0xff;
      if (value) chunks[3] = (chunks[3]! | STYLE_ATTRIBUTE_BITS[key as TextAttribute]!) & 0xff;
    }
  }
  chunks[1] = flags;
  const encoded = Uint8Array.from(chunks);
  if (encoded.byteLength > maximum) throw contentError("PAYLOAD_TOO_LARGE", "Source annotation payload is too large");
  return encoded;
}

function appendStyleString(output: number[], value: string, maximum: number): void {
  const bytes = encoder.encode(value);
  if (bytes.byteLength > 0xffff || output.length + 2 + bytes.byteLength > maximum) {
    throw contentError("PAYLOAD_TOO_LARGE", "semantic style string is too large");
  }
  output.push(bytes.byteLength & 0xff, bytes.byteLength >>> 8);
  output.push(...bytes);
}

function appendStyleColor(output: number[], color: ColorSpec, maximum: number): void {
  switch (color.type) {
    case "named": {
      const index = ANSI_COLOR_VALUES.indexOf(color.value);
      if (index < 0) throw contentError("INVALID_ANNOTATION_PAYLOAD", "unknown semantic ANSI color");
      output.push(1, index);
      break;
    }
    case "indexed":
      assertU32(color.value, "semantic indexed color");
      if (color.value > 255) throw contentError("INVALID_ANNOTATION_PAYLOAD", "semantic indexed color must fit in one byte");
      output.push(2, color.value);
      break;
    case "rgb":
      for (const [name, value] of [["red", color.r], ["green", color.g], ["blue", color.b]] as const) {
        if (!Number.isInteger(value) || value < 0 || value > 255) {
          throw contentError("INVALID_ANNOTATION_PAYLOAD", `semantic ${name} channel must fit in one byte`);
        }
      }
      output.push(3, color.r, color.g, color.b);
      break;
    case "theme": {
      const key = typeof color.key === "string" ? color.key : color.key.value;
      validateStyleName(key, "semantic theme key");
      output.push(4);
      appendStyleString(output, key, maximum);
      break;
    }
    default:
      throw contentError("INVALID_ANNOTATION_PAYLOAD", "unknown semantic style color");
  }
  if (output.length > maximum) throw contentError("PAYLOAD_TOO_LARGE", "Source annotation payload is too large");
}

function validateStyleName(value: string, label: string): void {
  if (typeof value !== "string" || value.length === 0 || /\\s|\\0/u.test(value)) {
    throw contentError("INVALID_ANNOTATION_PAYLOAD", `${label} must be non-empty and contain no whitespace or NUL`);
  }
}

/** Decodes the fixed semantic-style payload exposed by Source snapshots. */
export function decodeSemanticStylePayload(payload: Uint8Array): SemanticTextStyle {
  if (payload.length < 4 || payload[0] !== STYLE_PAYLOAD_VERSION) {
    throw contentError("ABI_MISMATCH", "semantic style payload version is unsupported");
  }
  const flags = payload[1]!;
  const presence = payload[2]!;
  const values = payload[3]!;
  if ((flags & ~0x0f) !== 0 || (presence & ~0x3f) !== 0 || (values & ~presence) !== 0) {
    throw contentError("ABI_MISMATCH", "semantic style payload is malformed");
  }
  let offset = 4;
  const readString = (label: string): string => {
    if (offset + 2 > payload.length) throw contentError("ABI_MISMATCH", `semantic style ${label} is truncated`);
    const length = payload[offset]! | (payload[offset + 1]! << 8);
    offset += 2;
    if (offset + length > payload.length) throw contentError("ABI_MISMATCH", `semantic style ${label} is truncated`);
    try {
      const value = new TextDecoder("utf-8", { fatal: true }).decode(payload.subarray(offset, offset + length));
      validateStyleName(value, label);
      offset += length;
      return value;
    } catch (error) {
      if (error instanceof TuiError) throw error;
      throw contentError("ABI_MISMATCH", `semantic style ${label} is not valid UTF-8`);
    }
  };
  const readByte = (label: string): number => {
    if (offset >= payload.length) throw contentError("ABI_MISMATCH", `semantic style ${label} is truncated`);
    return payload[offset++]!;
  };
  const readColor = (label: string): ColorSpec => {
    const kind = readByte(`${label} color`);
    if (kind === 1) {
      const index = readByte(`${label} ANSI color`);
      const value = ANSI_COLOR_VALUES[index];
      if (value === undefined) throw contentError("ABI_MISMATCH", `semantic style ${label} ANSI color is invalid`);
      return { type: "named", value };
    }
    if (kind === 2) return { type: "indexed", value: readByte(`${label} indexed color`) };
    if (kind === 3) return {
      type: "rgb",
      r: readByte(`${label} red`),
      g: readByte(`${label} green`),
      b: readByte(`${label} blue`),
    };
    if (kind === 4) return { type: "theme", key: readString(`${label} theme key`) };
    throw contentError("ABI_MISMATCH", `semantic style ${label} color kind is invalid`);
  };
  const role = flags & STYLE_FLAG_ROLE ? readString("role") : undefined;
  const foreground = flags & STYLE_FLAG_FOREGROUND ? readColor("foreground") : undefined;
  const background = flags & STYLE_FLAG_BACKGROUND ? readColor("background") : undefined;
  if (offset !== payload.length || (!(flags & STYLE_FLAG_ATTRIBUTES) && (presence !== 0 || values !== 0))) {
    throw contentError("ABI_MISMATCH", "semantic style payload has trailing or reserved data");
  }
  const attributes: Partial<Record<TextAttribute, boolean>> = {};
  for (const [name, bit] of Object.entries(STYLE_ATTRIBUTE_BITS) as [TextAttribute, number][]) {
    if (presence & bit) attributes[name] = (values & bit) !== 0;
  }
  return {
    ...(role === undefined ? {} : { role }),
    ...(foreground === undefined ? {} : { foreground }),
    ...(background === undefined ? {} : { background }),
    ...(flags & STYLE_FLAG_ATTRIBUTES ? { attributes } : {}),
  };
}

function boundary(bytes: Uint8Array, offset: number): boolean {
  return offset === 0 || offset === bytes.byteLength || (bytes[offset]! & 0xc0) !== 0x80;
}

function encodeAnnotations(bytes: Uint8Array, annotations: readonly TextSourceAnnotation[]): EncodedAnnotations {
  if (annotations.length > 16 * 1024) {
    throw contentError("LIMIT_EXCEEDED", "Source annotation count exceeds the configured limit");
  }
  const records = new Uint32Array(annotations.length * CONTENT_ABI_ANNOTATION_LANES);
  const payloads: Uint8Array[] = [];
  let payloadLength = 0;
  for (let index = 0; index < annotations.length; index += 1) {
    const annotation = annotations[index]!;
    validateAnnotationObject(annotation);
    const kind = annotation.kind === undefined ? "tag" : annotation.kind;
    if (typeof kind !== "string" || !Object.hasOwn(annotationKinds, kind)) {
      throw contentError("UNKNOWN_ANNOTATION_KIND", `unknown Source annotation kind ${JSON.stringify(kind)}`);
    }
    const annotationKind = kind as AnnotationKind;
    const start = annotation.startByte === undefined
      ? (annotationKind === "point" ? bytes.byteLength : 0)
      : annotation.startByte;
    const end = annotation.endByte === undefined
      ? (annotationKind === "point" ? start : bytes.byteLength)
      : annotation.endByte;
    assertU32(start, "annotation startByte");
    assertU32(end, "annotation endByte");
    if (start > end || end > bytes.byteLength || !boundary(bytes, start) || !boundary(bytes, end)) {
      throw contentError("INVALID_RANGE", "Source annotation range is not a UTF-8 boundary range");
    }
    if (annotationKind === "point" && start !== end) {
      throw contentError("INVALID_RANGE", "point annotations must have an empty range");
    }
    if (annotationKind !== "point" && start === end) {
      throw contentError("INVALID_RANGE", "non-point annotations must cover text");
    }
    const encoded = annotationPayload(
      annotation,
      annotationKind,
      MAX_ANNOTATION_PAYLOAD_BYTES - payloadLength,
    );
    const nextPayloadLength = payloadLength + encoded.byteLength;
    if (nextPayloadLength > MAX_ANNOTATION_PAYLOAD_BYTES || nextPayloadLength > MAX_U32) {
      throw contentError("PAYLOAD_TOO_LARGE", "Source annotation payload is too large");
    }
    const offset = index * CONTENT_ABI_ANNOTATION_LANES;
    records[offset] = annotationKinds[annotationKind];
    records[offset + 1] = 0;
    records[offset + 2] = start;
    records[offset + 3] = end;
    records[offset + 4] = payloadLength;
    records[offset + 5] = encoded.byteLength;
    records[offset + 6] = 0;
    records[offset + 7] = 0;
    payloads.push(encoded);
    payloadLength = nextPayloadLength;
  }
  const payload = new Uint8Array(payloadLength);
  let offset = 0;
  for (const part of payloads) {
    payload.set(part, offset);
    offset += part.byteLength;
  }
  return { records, payload };
}

function encodedText(text: string): Uint8Array {
  utf8ByteLength(
    text,
    MAX_SOURCE_PAYLOAD_BYTES,
    "Source UTF-8 payload exceeds the configured limit",
  );
  const bytes = encoder.encode(text);
  if (bytes.byteLength > MAX_SOURCE_PAYLOAD_BYTES || bytes.byteLength > MAX_U32) {
    throw contentError("PAYLOAD_TOO_LARGE", "Source UTF-8 payload exceeds the ABI limit");
  }
  return bytes;
}

function invokePayload(
  source: NativeTextSourceContract,
  text: string,
  annotations: readonly TextSourceAnnotation[],
  replace: boolean,
  requestWake: () => void,
): ReturnType<typeof finishMutation> {
  const bytes = encodedText(text);
  const encodedAnnotations = encodeAnnotations(bytes, annotations);
  const [environmentSlot, environmentGeneration, sourceSlot, sourceGeneration] = sourceIdentity(source);
  const result = new Uint32Array(CONTENT_ABI_MUTATION_RESULT_LANES);
  const symbols = session().symbols;
  const status = replace
    ? symbols.iyon_tui_source_replace_utf8_v1(
      environmentSlot,
      environmentGeneration,
      sourceSlot,
      sourceGeneration,
      bytes,
      bytes.byteLength,
      encodedAnnotations.records,
      annotations.length,
      encodedAnnotations.payload,
      encodedAnnotations.payload.byteLength,
      result,
    )
    : symbols.iyon_tui_source_append_utf8_v1(
      environmentSlot,
      environmentGeneration,
      sourceSlot,
      sourceGeneration,
      bytes,
      bytes.byteLength,
      encodedAnnotations.records,
      annotations.length,
      encodedAnnotations.payload,
      encodedAnnotations.payload.byteLength,
      result,
    );
  return finishMutation(status, result, requestWake);
}

function invokeNoPayload(
  source: NativeTextSourceContract,
  operation: "clear" | "seal" | "truncate",
  requestWake: () => void,
  offset?: bigint | number,
): ReturnType<typeof finishMutation> {
  const [environmentSlot, environmentGeneration, sourceSlot, sourceGeneration] = sourceIdentity(source);
  const result = new Uint32Array(CONTENT_ABI_MUTATION_RESULT_LANES);
  const symbols = session().symbols;
  let status: number;
  if (operation === "clear") {
    status = symbols.iyon_tui_source_clear_v1(
      environmentSlot,
      environmentGeneration,
      sourceSlot,
      sourceGeneration,
      result,
    );
  } else if (operation === "seal") {
    status = symbols.iyon_tui_source_seal_v1(
      environmentSlot,
      environmentGeneration,
      sourceSlot,
      sourceGeneration,
      result,
    );
  } else {
    const [offsetLow, offsetHigh] = splitU64(offset ?? 0n, "Source head");
    status = symbols.iyon_tui_source_head_truncate_v1(
      environmentSlot,
      environmentGeneration,
      sourceSlot,
      sourceGeneration,
      offsetLow,
      offsetHigh,
      result,
    );
  }
  return finishMutation(status, result, requestWake);
}

export function appendTextSource(
  source: NativeTextSourceContract,
  text: string,
  annotations: readonly TextSourceAnnotation[],
  requestWake: () => void,
): ReturnType<typeof finishMutation> {
  return invokePayload(source, text, annotations, false, requestWake);
}

export function replaceTextSource(
  source: NativeTextSourceContract,
  text: string,
  annotations: readonly TextSourceAnnotation[],
  requestWake: () => void,
): ReturnType<typeof finishMutation> {
  return invokePayload(source, text, annotations, true, requestWake);
}

export function clearTextSource(
  source: NativeTextSourceContract,
  requestWake: () => void,
): ReturnType<typeof finishMutation> {
  return invokeNoPayload(source, "clear", requestWake);
}

export function sealTextSource(
  source: NativeTextSourceContract,
  requestWake: () => void,
): ReturnType<typeof finishMutation> {
  return invokeNoPayload(source, "seal", requestWake);
}

export function truncateTextSource(
  source: NativeTextSourceContract,
  offset: bigint | number,
  requestWake: () => void,
): ReturnType<typeof finishMutation> {
  return invokeNoPayload(source, "truncate", requestWake, offset);
}

export function contentFfiMetadata(): Readonly<{ readonly artifactPath: string; readonly metadata: Uint32Array }> {
  const current = session();
  return { artifactPath: current.artifactPath, metadata: current.metadata.slice() };
}

export const CONTENT_FFI_SCHEMA = Object.freeze({
  metadataBytes: CONTENT_ABI_METADATA_BYTES,
  annotationBytes: CONTENT_ABI_ANNOTATION_BYTES,
  mutationResultBytes: CONTENT_ABI_MUTATION_RESULT_BYTES,
});
