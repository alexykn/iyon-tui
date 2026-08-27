import { HandleBase, nativeComponentIdOf, registerNativeResource } from "./handles.ts";
import type { NativeTextInputContract, NativeTuiOutputContract } from "./native.ts";
import { Output } from "./types.ts";
import type { TextInput as TextInputContract } from "./types.ts";
import { componentViewFor } from "./component-facade.ts";
import { View } from "./values/view.ts";

const TEXT_INPUT_NATIVE_TOKEN = Symbol("text-input-native-construction");

/**
 * Host-bound text input. Construct it with `Tui.createTextInput()` so its
 * border, component mount, paste routing, and retained runtime all come from
 * the same host-owned path.
 *
 * The caller may dispose the handle early; the owning Tui also disposes
 * factory-created inputs during `close()`/`exit()`. Detached construction is
 * intentionally not supported because a detached input cannot faithfully
 * provide the host-bound border/component semantics. Each input has one
 * mounted component identity; duplicate View component nodes are rejected.
 */
export class TextInput extends HandleBase<"text-input"> implements TextInputContract {
  private submittedOutput?: Output<string>;

  private constructor(nativeHandle: never, token?: typeof TEXT_INPUT_NATIVE_TOKEN) {
    if (token !== TEXT_INPUT_NATIVE_TOKEN) throw new TypeError("TextInput native construction is private");
    super("text-input", nativeHandle as never);
  }

  text(): string { return this.call(() => this.nativeAs<NativeTextInputContract>().text()); }
  cursorBytes(): number { return this.call(() => this.nativeAs<NativeTextInputContract>().cursorBytes()); }
  setText(value: string): void { this.call(() => this.nativeAs<NativeTextInputContract>().setText(value)); }
  clear(): void { this.call(() => this.nativeAs<NativeTextInputContract>().clear()); }
  submitted(): Output<string> {
    return this.call(() => {
      // Rust exposes one stable channel per input. Cache the facade so
      // repeated calls preserve that channel identity on the JS side too.
      if (this.submittedOutput === undefined) {
        this.submittedOutput = new NativeOutput(this.nativeAs<NativeTextInputContract>().submitted() as never);
      }
      return this.submittedOutput;
    });
  }
  setMultiline(enabled: boolean): void { this.call(() => this.nativeAs<NativeTextInputContract>().setMultiline(enabled)); }
  isMultiline(): boolean { return this.call(() => this.nativeAs<NativeTextInputContract>().isMultiline()); }
  view(): View {
    this.ensureOpen();
    return nativeComponentIdOf(this) === undefined ? View.spacer(0) : componentViewFor(this);
  }

}

/** @internal Creates the only supported TextInput construction path. */
export function createTextInput(nativeHandle: never): TextInput {
  const Constructor = TextInput as unknown as new (nativeHandle: never, token: typeof TEXT_INPUT_NATIVE_TOKEN) => TextInput;
  return new Constructor(nativeHandle, TEXT_INPUT_NATIVE_TOKEN);
}

class NativeOutput<T> extends Output<T> {
  /** @internal Native channel construction overload. */
  constructor(resource: never);
  constructor(resource: NativeTuiOutputContract) {
    super();
    registerNativeResource(this, resource);
  }
}
