import { HandleBase, nativeComponentIdOf, registerNativeResource } from "./handles.ts";
import { nativeTui } from "./native-handles.ts";
import type { NativeTextInputContract, NativeTuiOutputContract } from "./native.ts";
import { OutputHandle } from "./types.ts";
import type { TextInput as TextInputContract, TextInputOptions } from "./types.ts";
import { View } from "./values/view.ts";

export class TextInput extends HandleBase<"text-input"> implements TextInputContract {
  constructor(options?: TextInputOptions);
  /** @internal Native host construction overload; consumers cannot provide a `never` value. */
  constructor(options: TextInputOptions | undefined, nativeHandle: never);
  constructor(options: TextInputOptions = {}, nativeHandle?: NativeTextInputContract) {
    super("text-input", (nativeHandle ?? nativeTui.textInput(options.multiline)) as never);
  }

  text(): string { return this.call(() => this.nativeAs<NativeTextInputContract>().text()); }
  cursorBytes(): number { return this.call(() => this.nativeAs<NativeTextInputContract>().cursorBytes()); }
  setText(value: string): void { this.call(() => this.nativeAs<NativeTextInputContract>().setText(value)); }
  clear(): void { this.call(() => this.nativeAs<NativeTextInputContract>().clear()); }
  submitted(): OutputHandle<string> {
    return this.call(() => new NativeOutputHandle(this.nativeAs<NativeTextInputContract>().submitted() as never));
  }
  setMultiline(enabled: boolean): void { this.call(() => this.nativeAs<NativeTextInputContract>().setMultiline(enabled)); }
  isMultiline(): boolean { return this.call(() => this.nativeAs<NativeTextInputContract>().isMultiline()); }
  view(): View {
    this.ensureOpen();
    return nativeComponentIdOf(this) === undefined ? View.spacer(0) : View.component(this);
  }

}

class NativeOutputHandle<T> extends OutputHandle<T> {
  /** @internal Native channel construction overload. */
  constructor(resource: never);
  constructor(resource: NativeTuiOutputContract) {
    super();
    registerNativeResource(this, resource);
  }
}
