import { HandleBase, nativeTui } from "./handles.ts";
import type { TextInput as TextInputContract, OutputHandle } from "./types.ts";
import { View } from "./values/view.ts";
import type { BorderNode } from "./ir.ts";
import type { NativeTuiOutputContract } from "./native.ts";

export class TextInput extends HandleBase<ReturnType<typeof nativeTui.textInput>, "text-input"> implements TextInputContract {
  constructor(options?: { multiline?: boolean; border?: BorderNode }, nativeHandle = nativeTui.textInput(options?.multiline)) { super("text-input", nativeHandle); }

  text(): string { return this.call(() => this.nativeHandle.text()); }
  cursorBytes(): number { return this.call(() => this.nativeHandle.cursorBytes()); }
  setText(value: string): void { this.call(() => this.nativeHandle.setText(value)); }
  clear(): void { this.call(() => this.nativeHandle.clear()); }
  submitted(): OutputHandle<string> {
    return this.call(() => new NativeOutputHandle(this.nativeHandle.submitted() as NativeTuiOutputContract));
  }
  setMultiline(enabled: boolean): void { this.call(() => this.nativeHandle.setMultiline(enabled)); }
  isMultiline(): boolean { return this.call(() => this.nativeHandle.isMultiline()); }
  view(): View {
    this.ensureOpen();
    return this.nativeComponentId() === undefined ? View.spacer(0) : View.component(this);
  }

  nativeComponentId(): number | undefined {
    const id = this.nativeHandle.componentId?.();
    return id === null || id === undefined ? undefined : id;
  }
}

export class NativeOutputHandle<T> implements OutputHandle<T> {
  readonly kind = "output" as const;
  readonly payload!: T;
  constructor(readonly nativeObject: NativeTuiOutputContract) {}
}
