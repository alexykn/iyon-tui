import { registerNativeResource } from "./handles.ts";
import type { NativeTextInputContract, NativeTuiOutputContract } from "./native.ts";
import { FrameworkHandle, Output } from "./types.ts";
import type { TextInput as TextInputContract } from "./types.ts";
import { composeComponent } from "./composition/compose.ts";
import type { View } from "./api/view/view.ts";

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
const outputInputs = new WeakMap<object, WeakRef<TextInput>>();

export class TextInput extends FrameworkHandle<"text-input"> implements TextInputContract {
  private submittedOutput?: Output<string>;

  private constructor(resource: never, token?: typeof TEXT_INPUT_NATIVE_TOKEN) {
    if (token !== TEXT_INPUT_NATIVE_TOKEN) throw new TypeError("TextInput native construction is private");
    super("text-input", resource as never);
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
        this.submittedOutput = createNativeOutput(this.nativeAs<NativeTextInputContract>().submitted());
        outputInputs.set(this.submittedOutput, new WeakRef(this));
      }
      return this.submittedOutput;
    });
  }
  setMultiline(enabled: boolean): void { this.call(() => this.nativeAs<NativeTextInputContract>().setMultiline(enabled)); }
  isMultiline(): boolean { return this.call(() => this.nativeAs<NativeTextInputContract>().isMultiline()); }
  view(): View {
    this.ensureOpen();
    return composeComponent(this);
  }

}

/** @internal Creates the only supported TextInput construction path. */
export function createTextInput(resource: never): TextInput {
  const Constructor = TextInput as unknown as new (resource: never, token: typeof TEXT_INPUT_NATIVE_TOKEN) => TextInput;
  return new Constructor(resource, TEXT_INPUT_NATIVE_TOKEN);
}

/** @internal Identifies the TextInput that owns a native output channel. */
export function textInputForOutput(output: object): TextInput | undefined {
  return outputInputs.get(output)?.deref();
}

function createNativeOutput<T>(resource: NativeTuiOutputContract): Output<T> {
  // Output has a private constructor so consumers cannot manufacture a
  // channel that lacks native routing identity. The native-backed instance is
  // created here without adding another public implementation class.
  const output = Object.create(Output.prototype) as Output<T>;
  Object.defineProperty(output, "kind", { value: "output", enumerable: true });
  registerNativeResource(output, resource);
  return output;
}
