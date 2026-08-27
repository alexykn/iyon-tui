import { HandleBase } from "./handles.ts";
import { nativeTui } from "./native-handles.ts";
import type { NativeTextStreamContract } from "./native.ts";
import type { StreamAnnotation, StreamSnapshot, TextStream as TextStreamContract, TextStreamOptions } from "./types.ts";

/**
 * Independent mutable text source. `new TextStream()` is the canonical
 * construction path; the caller owns and disposes it, and it may outlive any
 * Tui. Attaching it to History gives the host a live view of the same source;
 * stream updates do not create a second stream or a second retained View path.
 * A stream is intended to have one active History attachment at a time.
 */
export class TextStream extends HandleBase<"text-stream"> implements TextStreamContract {
  constructor(options: TextStreamOptions = {}) { super("text-stream", nativeTui.textStream(options) as never); }
  update(text: string): void { this.call(() => this.nativeAs<NativeTextStreamContract>().update(text)); }
  append(text: string, annotations: readonly StreamAnnotation[] = []): void {
    this.call(() => this.nativeAs<NativeTextStreamContract>().append(text, annotations));
  }
  seal(): void { this.call(() => this.nativeAs<NativeTextStreamContract>().seal()); }
  snapshot(): StreamSnapshot { return this.call(() => this.nativeAs<NativeTextStreamContract>().snapshot() as StreamSnapshot); }
}

export { TextStream as StreamPane };
