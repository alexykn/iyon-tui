import { native } from "./native.ts";
import { asTuiError, tuiError } from "./errors.ts";
import type { NativeHandle, NativeHandleId, TextStreamOptions } from "./types.ts";

export interface NativeHandleObject {
  dispose(): void;
}

let nextHandleId = 1;

export abstract class HandleBase<T extends NativeHandleObject, K extends string = string> implements NativeHandle {
  readonly id = nextHandleId++ as NativeHandleId;
  private isDisposed = false;

  protected constructor(readonly kind: K, protected readonly nativeHandle: T) {}

  get disposed(): boolean { return this.isDisposed; }

  dispose(): void {
    if (this.isDisposed) return;
    this.isDisposed = true;
    try {
      this.nativeHandle.dispose();
    } catch (error) {
      throw asTuiError(error);
    }
  }

  protected ensureOpen(): void {
    if (this.isDisposed) throw tuiError("disposed-handle", `${this.kind} handle has been disposed`, { id: this.id });
  }

  protected call<R>(operation: () => R): R {
    try {
      this.ensureOpen();
      return operation();
    } catch (error) {
      throw asTuiError(error);
    }
  }
}

export function requireNativeClass<T>(factory: T | undefined, name: string): T {
  if (factory === undefined) throw tuiError("runtime", `${name} is unavailable in the native addon`);
  return factory;
}

export const nativeTui = {
  history: () => new (requireNativeClass(native.NativeHistory, "NativeHistory"))(),
  textInput: (multiline?: boolean) => new (requireNativeClass(native.NativeTextInput, "NativeTextInput"))(multiline),
  textStream: (options?: TextStreamOptions) => new (requireNativeClass(native.NativeTextStream, "NativeTextStream"))(options),
  markdownProjector: () => new (requireNativeClass(native.NativeMarkdownProjector, "NativeMarkdownProjector"))(),
  plainProjector: () => new (requireNativeClass(native.NativePlainProjector, "NativePlainProjector"))(),
};
