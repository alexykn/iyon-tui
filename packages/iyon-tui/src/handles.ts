import { asTuiError, tuiError } from "./errors.ts";
import { FrameworkHandle } from "./types.ts";
import type { HandleId } from "./types.ts";

interface DisposableResource {
  dispose(): void;
}

const nativeResources = new WeakMap<object, object>();
const handleIds = new WeakMap<object, HandleId>();
let nextHandleId = 1;

/**
 * Private registry for native-backed framework handles. Native addon objects
 * never become public properties of a handle, and all cross-handle unwrapping
 * goes through this module.
 */
export function registerNativeResource(handle: object, resource: object): void {
  if (nativeResources.has(handle)) throw tuiError("runtime", "framework handle already has a native resource");
  nativeResources.set(handle, resource);
}

export function nativeResourceOf<T extends object>(handle: object): T {
  const resource = nativeResources.get(handle);
  if (resource === undefined) throw tuiError("disposed-handle", "framework handle has no live native resource");
  return resource as T;
}

export function handleIdOf(handle: object): HandleId {
  const id = handleIds.get(handle);
  if (id === undefined) throw tuiError("disposed-handle", "framework handle has no local identity");
  return id;
}

/** Reads an optional native component identity without consulting public methods. */
export function nativeComponentIdOf(handle: object): number | undefined {
  const resource = nativeResourceOf<{ componentId?: () => number | null }>(handle);
  const id = resource.componentId?.();
  return id === null || id === undefined ? undefined : id;
}

/** Resolves the retained component identity without consulting public methods. */
export function componentIdOf(handle: object): HandleId {
  return (nativeComponentIdOf(handle) ?? handleIdOf(handle)) as HandleId;
}

export abstract class HandleBase<K extends string = string> extends FrameworkHandle {
  readonly id: HandleId;
  private isDisposed = false;

  protected constructor(readonly kind: K, nativeHandle: never) {
    super();
    this.id = nextHandleId++ as HandleId;
    handleIds.set(this, this.id);
    registerNativeResource(this, nativeHandle as unknown as object);
  }

  protected nativeAs<T extends object>(): T {
    return nativeResourceOf<T>(this);
  }

  get disposed(): boolean { return this.isDisposed; }

  dispose(): void {
    if (this.isDisposed) return;
    this.isDisposed = true;
    try {
      nativeResourceOf<DisposableResource>(this).dispose();
    } catch (error) {
      throw asTuiError(error);
    } finally {
      nativeResources.delete(this);
      handleIds.delete(this);
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
