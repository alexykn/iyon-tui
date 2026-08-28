import { tuiError } from "../../api/errors.ts";

interface DisposableResource {
  dispose(): void;
}

/**
 * Private association between an opaque framework value and its native
 * resource. Runtime handle identity and lifetime are owned by
 * runtime/handle-registry.ts; this module only owns the raw resource map.
 */
const nativeResources = new WeakMap<object, object>();

/** Registers a native resource for an opaque framework value. */
export function registerNativeResource(handle: object, resource: object): void {
  if (nativeResources.has(handle)) throw tuiError("runtime", "framework value already has a native resource");
  nativeResources.set(handle, resource);
}

/** Retrieves the raw native resource for a live framework value. */
export function nativeResourceOf<T extends object>(handle: object): T {
  const resource = nativeResources.get(handle);
  if (resource === undefined) throw tuiError("disposed-handle", "framework value has no live native resource");
  return resource as T;
}

/** Removes a value's raw native association after native disposal. */
export function releaseNativeResource(handle: object): void {
  nativeResources.delete(handle);
}

/** Disposes the raw native resource owned by a framework value. */
export function disposeNativeResource(handle: object): void {
  nativeResourceOf<DisposableResource>(handle).dispose();
}
