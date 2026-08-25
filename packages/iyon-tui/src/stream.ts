import { HandleBase, nativeTui } from "./handles.ts";
import type { StreamAnnotation, StreamSnapshot, TextStream as TextStreamContract, TextStreamOptions } from "./types.ts";

export class TextStream extends HandleBase<ReturnType<typeof nativeTui.textStream>, "text-stream"> implements TextStreamContract {
  constructor(options: TextStreamOptions = {}) { super("text-stream", nativeTui.textStream(options)); }
  update(text: string): void { this.call(() => this.nativeHandle.update(text)); }
  append(text: string, annotations: readonly StreamAnnotation[] = []): void {
    this.call(() => this.nativeHandle.append(text, annotations));
  }
  seal(): void { this.call(() => this.nativeHandle.seal()); }
  snapshot(): StreamSnapshot { return this.call(() => this.nativeHandle.snapshot() as StreamSnapshot); }
  nativeObject(): object { this.ensureOpen(); return this.nativeHandle; }
}

export { TextStream as StreamPane };
