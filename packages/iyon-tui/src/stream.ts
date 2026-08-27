import { FrameworkHandle } from "./types.ts";
import { assertTextStreamUsable } from "./history.ts";
import { nativeTui } from "./native-handles.ts";
import type { NativeTextStreamContract } from "./native.ts";
import type { StreamAnnotation, StreamSnapshot, TextStream as TextStreamContract, TextStreamOptions } from "./types.ts";

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
