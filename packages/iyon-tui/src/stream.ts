import { HandleBase } from "./handles.ts";
import { nativeTui } from "./native-handles.ts";
import type { NativeTextStreamContract } from "./native.ts";
import type { StreamAnnotation, StreamSnapshot, TextStream as TextStreamContract, TextStreamOptions } from "./types.ts";

export class TextStream extends HandleBase<"text-stream"> implements TextStreamContract {
  constructor(options: TextStreamOptions = {}) { super("text-stream", nativeTui.textStream(options) as never); }
  update(text: string): void { this.call(() => this.nativeAs<NativeTextStreamContract>().update(text)); }
  append(text: string, annotations: readonly StreamAnnotation[] = []): void {
    this.call(() => this.nativeAs<NativeTextStreamContract>().append(text, annotations));
  }
  seal(): void { this.call(() => this.nativeAs<NativeTextStreamContract>().seal()); }
  snapshot(): StreamSnapshot { return this.call(() => this.nativeAs<NativeTextStreamContract>().snapshot() as StreamSnapshot); }
  nativeObject(): object { this.ensureOpen(); return this.nativeAs<NativeTextStreamContract>(); }
}

export { TextStream as StreamPane };
