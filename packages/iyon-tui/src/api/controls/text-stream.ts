import { FrameworkHandle } from "./framework-handle.ts";
import { assertTextStreamUsable } from "./history.ts";
import { nativeTui } from "../../transport/native/factories.ts";
import type { NativeTextStreamContract } from "../../transport/native/addon.ts";
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

export interface TextStream extends FrameworkHandle<"text-stream"> {
  readonly kind: "text-stream";
  update(text: string): void;
  append(text: string, annotations?: readonly StreamAnnotation[]): void;
  seal(): void;
  snapshot(): StreamSnapshot;
}

type TextStreamContract = TextStream;

/**
 * Independent mutable text source. `new TextStream()` is the canonical
 * construction path; the caller owns and disposes it, and a detached stream
 * may outlive any Tui. Attaching it to History gives the host a live view of the same source;
 * stream updates do not create a second stream or a second retained View path.
 * A stream is intended to have one active History attachment at a time.
 */
export class TextStream extends FrameworkHandle<"text-stream"> implements TextStreamContract {
  constructor(options: TextStreamOptions = {}) { super("text-stream", nativeTui.textStream(options) as never); }
  update(text: string): void {
    this.callSource(() => this.nativeAs<NativeTextStreamContract>().update(text));
  }
  append(text: string, annotations: readonly StreamAnnotation[] = []): void {
    this.callSource(() => this.nativeAs<NativeTextStreamContract>().append(text, annotations));
  }
  seal(): void { this.callSource(() => this.nativeAs<NativeTextStreamContract>().seal()); }
  snapshot(): StreamSnapshot {
    return this.callSource(() => this.nativeAs<NativeTextStreamContract>().snapshot() as StreamSnapshot);
  }

  private callSource<R>(operation: () => R): R {
    return this.call(() => {
      assertTextStreamUsable(this);
      return operation();
    });
  }
}
