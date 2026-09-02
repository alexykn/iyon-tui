import { FrameworkHandle } from "./framework-handle.ts";
import { assertTextStreamUsable } from "./history.ts";
import { nativeTui } from "../../transport/native/factories.ts";
import type { NativeTextSourceContract } from "../../transport/native/addon.ts";
import {
  appendTextSource,
  replaceTextSource,
  sealTextSource,
} from "../../transport/content/ffi.ts";
import { runtimeEnvironment } from "../../runtime/environment.ts";
import type { StreamAnnotation, StreamSnapshot } from "../content/stream-snapshot.ts";

export interface TextStreamPresentation {
  readonly insets?: { readonly top?: number; readonly right?: number; readonly bottom?: number; readonly left?: number };
}

export interface TextStreamPacing {
  readonly tickIntervalMs?: number;
  readonly spring?: number;
  readonly minUnitsPerSecond?: number;
  readonly maxUnitsPerSecond?: number;
}

export interface TextStreamOptions {
  readonly projector?: "markdown";
  readonly presentation?: TextStreamPresentation;
  readonly pacing?: TextStreamPacing;
}

/** Typed control values used only when this compatibility source is mounted by History. */
export interface TextStreamControl {
  readonly projector: "plain" | "markdown";
  readonly smooth: boolean;
  readonly tickIntervalMs: number;
  readonly spring: number;
  readonly minUnitsPerSecond: number;
  readonly maxUnitsPerSecond: number;
  readonly insets: { readonly top: number; readonly right: number; readonly bottom: number; readonly left: number };
}

export interface TextStream extends FrameworkHandle<"text-stream"> {
  readonly kind: "text-stream";
  update(text: string): void;
  append(text: string, annotations?: readonly StreamAnnotation[]): void;
  seal(): void;
  snapshot(): StreamSnapshot;
}

type TextStreamContract = TextStream;
const textStreamControls = new WeakMap<TextStream, TextStreamControl>();

/**
 * Independent mutable text source. `new TextStream()` is the canonical
 * construction path; the caller owns and disposes it, and a detached stream
 * may outlive any Tui. Attaching it to History gives the host a live view of the same source;
 * stream updates do not create a second stream or a second retained View path.
 * A stream is intended to have one active History attachment at a time.
 */
export class TextStream extends FrameworkHandle<"text-stream"> implements TextStreamContract {
  constructor(options: TextStreamOptions = {}) {
    const controls = normalizeTextStreamOptions(options);
    super("text-stream", nativeTui.textSource("stream") as never);
    textStreamControls.set(this, controls);
  }

  /** Compatibility update; the new authoritative operation is Source.replace. */
  update(text: string): void {
    this.callSource(() => replaceTextSource(this.nativeSource(), text, [], sourceWake));
  }

  append(text: string, annotations: readonly StreamAnnotation[] = []): void {
    this.callSource(() => appendTextSource(
      this.nativeSource(),
      text,
      annotations.map(({ namespace, name }) => ({ kind: "tag" as const, namespace, name })),
      sourceWake,
    ));
  }

  seal(): void {
    this.callSource(() => sealTextSource(this.nativeSource(), sourceWake));
  }

  override dispose(): void {
    if (this.disposed) return;
    this.nativeSource().requestDisposeWhenUnused();
    super.dispose();
  }

  snapshot(): StreamSnapshot {
    return this.callSource(() => {
      const source = this.nativeSource().snapshot() as {
        readonly text: string;
        readonly revision: string | number;
        readonly sealed: boolean;
        readonly sourceBase?: string | number;
        readonly annotations?: readonly {
          readonly kind: number;
          readonly startByte: string | number;
          readonly endByte: string | number;
          readonly payload?: readonly number[];
        }[];
      };
      const revision = typeof source.revision === "number" ? source.revision : Number(source.revision);
      if (!Number.isSafeInteger(revision)) throw new Error("TextStream revision exceeds the compatibility number range");
      const segments = compatibilitySegments(source);
      return {
        text: source.text,
        revision,
        sealed: source.sealed,
        ...(segments.length === 0 ? {} : { segments }),
      };
    });
  }

  private nativeSource(): NativeTextSourceContract {
    return this.nativeAs<NativeTextSourceContract>();
  }

  private callSource<R>(operation: () => R): R {
    return this.call(() => {
      assertTextStreamUsable(this);
      return operation();
    });
  }
}

function sourceWake(): void {
  runtimeEnvironment().wakeBroker.markEnvironmentPending();
}

function compatibilitySegments(source: {
  readonly text: string;
  readonly sourceBase?: string | number;
  readonly annotations?: readonly {
    readonly kind: number;
    readonly startByte: string | number;
    readonly endByte: string | number;
    readonly payload?: readonly number[];
  }[];
}): readonly { readonly annotations: readonly StreamAnnotation[]; readonly text: string }[] {
  const tags = (source.annotations ?? [])
    .filter((annotation) => annotation.kind === 1)
    .map((annotation) => ({
      start: Number(annotation.startByte) - Number(source.sourceBase ?? 0),
      end: Number(annotation.endByte) - Number(source.sourceBase ?? 0),
      tag: decodeStreamTag(annotation.payload ?? []),
    }))
    .filter((annotation) => Number.isSafeInteger(annotation.start) && Number.isSafeInteger(annotation.end)
      && annotation.start >= 0 && annotation.end > annotation.start && annotation.tag !== undefined);
  if (tags.length === 0) return [];
  const byteLength = new TextEncoder().encode(source.text).byteLength;
  const boundaries = new Set<number>([0, byteLength]);
  for (const annotation of tags) {
    if (annotation.end <= byteLength) {
      boundaries.add(annotation.start);
      boundaries.add(annotation.end);
    }
  }
  const ordered = [...boundaries].sort((left, right) => left - right);
  const output: Array<{ annotations: readonly StreamAnnotation[]; text: string }> = [];
  for (const [index, start] of ordered.entries()) {
    const end = ordered[index + 1];
    if (end === undefined || end <= start) continue;
    const active = tags
      .filter((annotation) => annotation.start < end && annotation.end > start)
      .map((annotation) => annotation.tag!);
    const text = source.text.slice(codeUnitAtUtf8Byte(source.text, start), codeUnitAtUtf8Byte(source.text, end));
    if (text.length === 0) continue;
    const previous = output.at(-1);
    if (previous !== undefined && sameStreamTags(previous.annotations, active)) {
      output[output.length - 1] = { annotations: previous.annotations, text: previous.text + text };
    } else {
      output.push({ annotations: active, text });
    }
  }
  return output;
}

function decodeStreamTag(payload: readonly number[]): StreamAnnotation | undefined {
  const separator = payload.indexOf(0);
  if (separator < 1 || separator === payload.length - 1) return undefined;
  try {
    const decoder = new TextDecoder("utf-8", { fatal: true });
    const namespace = decoder.decode(Uint8Array.from(payload.slice(0, separator)));
    const name = decoder.decode(Uint8Array.from(payload.slice(separator + 1)));
    return { namespace, name };
  } catch {
    return undefined;
  }
}

function codeUnitAtUtf8Byte(text: string, byteOffset: number): number {
  if (byteOffset <= 0) return 0;
  let bytes = 0;
  for (let index = 0; index < text.length; index += 1) {
    const code = text.charCodeAt(index);
    const width = code >= 0xd800 && code <= 0xdbff
      && index + 1 < text.length
      && text.charCodeAt(index + 1) >= 0xdc00
      && text.charCodeAt(index + 1) <= 0xdfff ? 4
      : code <= 0x7f ? 1 : code <= 0x7ff ? 2 : 3;
    if (bytes + width > byteOffset) return index;
    bytes += width;
    if (width === 4) index += 1;
    if (bytes === byteOffset) return index + 1;
  }
  return text.length;
}

function sameStreamTags(left: readonly StreamAnnotation[], right: readonly StreamAnnotation[]): boolean {
  return left.length === right.length
    && left.every((tag, index) => tag.namespace === right[index]?.namespace && tag.name === right[index]?.name);
}

export function textStreamControlFor(stream: TextStreamContract): TextStreamControl {
  if (!(stream instanceof TextStream)) throw new TypeError("History.pushStream requires a framework TextStream");
  const controls = textStreamControls.get(stream);
  if (controls === undefined) throw new TypeError("TextStream control state is unavailable");
  return controls;
}

function normalizeTextStreamOptions(options: TextStreamOptions): TextStreamControl {
  if (typeof options !== "object" || options === null) throw new TypeError("TextStream options must be an object");
  for (const key of Object.keys(options)) {
    if (key !== "projector" && key !== "presentation" && key !== "pacing") {
      throw new RangeError(`unknown TextStream option ${JSON.stringify(key)}`);
    }
  }
  if (options.projector !== undefined && options.projector !== "markdown") {
    throw new RangeError("TextStream projector must be markdown");
  }
  const pacing = options.pacing ?? {};
  for (const key of Object.keys(pacing)) {
    if (key !== "tickIntervalMs" && key !== "spring" && key !== "minUnitsPerSecond" && key !== "maxUnitsPerSecond") {
      throw new RangeError(`unknown TextStream pacing option ${JSON.stringify(key)}`);
    }
  }
  const tickIntervalMs = pacing.tickIntervalMs ?? 16;
  const spring = pacing.spring ?? 2;
  const minUnitsPerSecond = pacing.minUnitsPerSecond ?? 20;
  const maxUnitsPerSecond = pacing.maxUnitsPerSecond ?? 800;
  if (!Number.isInteger(tickIntervalMs) || tickIntervalMs <= 0) throw new RangeError("TextStream tickIntervalMs must be a positive integer");
  for (const [name, value] of [["spring", spring], ["minUnitsPerSecond", minUnitsPerSecond], ["maxUnitsPerSecond", maxUnitsPerSecond]] as const) {
    if (!Number.isFinite(value) || value < 0) throw new RangeError(`TextStream ${name} must be finite and non-negative`);
  }
  if (minUnitsPerSecond > maxUnitsPerSecond || maxUnitsPerSecond === 0) {
    throw new RangeError("TextStream pacing rates are invalid");
  }
  const insets = options.presentation?.insets ?? {};
  const normalizedInsets = {
    top: insets.top ?? 0,
    right: insets.right ?? 0,
    bottom: insets.bottom ?? 0,
    left: insets.left ?? 0,
  };
  for (const [name, value] of Object.entries(normalizedInsets)) {
    if (!Number.isInteger(value) || value < 0 || value > 65535) throw new RangeError(`TextStream inset ${name} is invalid`);
  }
  return Object.freeze({
    projector: options.projector === "markdown" ? "markdown" : "plain",
    smooth: options.projector === "markdown",
    tickIntervalMs,
    spring,
    minUnitsPerSecond,
    maxUnitsPerSecond,
    insets: Object.freeze(normalizedInsets),
  });
}
