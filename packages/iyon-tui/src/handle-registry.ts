import { asTuiError, tuiError } from "./errors.ts";
import type { HandleId } from "./types.ts";

interface DisposableResource {
  dispose(): void;
}

const nativeResources = new WeakMap<object, object>();
const handleIds = new WeakMap<object, HandleId>();
let nextHandleId = 1;

/** Registers the native resource owned by one framework handle. */
export function registerFrameworkHandle(handle: object, resource: object): HandleId {
  if (nextHandleId > Number.MAX_SAFE_INTEGER) throw new Error("TUI framework handle identity exhausted");
  const id = nextHandleId++ as HandleId;
  try {
    handleIds.set(handle, id);
    registerNativeResource(handle, resource);
    return id;
  } catch (error) {
    handleIds.delete(handle);
    throw error;
  }
}

/** Registers a native resource for an opaque non-handle value such as Output. */
export function registerNativeResource(handle: object, resource: object): void {
  if (nativeResources.has(handle)) throw tuiError("runtime", "framework value already has a native resource");
  nativeResources.set(handle, resource);
}

export function nativeResourceOf<T extends object>(handle: object): T {
  const resource = nativeResources.get(handle);
  if (resource === undefined) throw tuiError("disposed-handle", "framework value has no live native resource");
  return resource as T;
}

export function handleIdOf(handle: object): HandleId {
  const id = handleIds.get(handle);
  if (id === undefined) throw tuiError("disposed-handle", "framework handle has no local identity");
  return id;
}

/** Removes all private identity/resource associations after native disposal. */
export function releaseFrameworkHandle(handle: object): void {
  nativeResources.delete(handle);
  handleIds.delete(handle);
}

/** Shared lifecycle implementation for nominal framework-owned handles. */
export function disposeFrameworkResource(handle: object): void {
  try {
    nativeResourceOf<DisposableResource>(handle).dispose();
  } catch (error) {
    throw asTuiError(error);
  } finally {
    releaseFrameworkHandle(handle);
  }
}
