import { asTuiError, tuiError } from "./errors.ts";
import type { NativeHandle, NativeHandleId } from "./types.ts";

interface HandleResource {
  dispose(): void;
}

let nextHandleId = 1;

export abstract class HandleBase<K extends string = string> implements NativeHandle {
  readonly id = nextHandleId++ as NativeHandleId;
  private isDisposed = false;
  #nativeHandle: HandleResource;

  protected constructor(readonly kind: K, nativeHandle: never) {
    this.#nativeHandle = nativeHandle as unknown as HandleResource;
  }

  protected nativeAs<T>(): T {
    return this.#nativeHandle as T;
  }

  get disposed(): boolean { return this.isDisposed; }

  dispose(): void {
    if (this.isDisposed) return;
    this.isDisposed = true;
    try {
      this.#nativeHandle.dispose();
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
