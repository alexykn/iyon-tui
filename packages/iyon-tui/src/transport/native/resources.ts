import { tuiError } from "../../api/errors.ts";
import type { HandleId } from "../../api/controls/framework-handle.ts";

interface DisposableResource {
  dispose(): void;
}

/**
 * Private association between an opaque framework value and its native
 * resource. Runtime handle identity and lifetime are owned by
 * runtime/handle-registry.ts; this module only owns the raw resource map.
 */
const nativeResources = new WeakMap<object, object>();
const nativeResourcesByHandleId = new Map<HandleId, WeakRef<object>>();

/** Registers a native resource for an opaque framework value. */
export function registerNativeResource(handle: object, resource: object, handleId?: HandleId): void {
  if (nativeResources.has(handle)) throw tuiError("runtime", "framework value already has a native resource");
  if (handleId !== undefined && nativeResourcesByHandleId.has(handleId)) {
    throw tuiError("runtime", "framework handle identity already has a native resource", { id: handleId });
  }
  nativeResources.set(handle, resource);
  if (handleId !== undefined) nativeResourcesByHandleId.set(handleId, new WeakRef(resource));
}

/** Retrieves a raw native resource through its semantic framework identity. */
export function nativeResourceForHandleId<T extends object>(handleId: HandleId): T {
  const resource = nativeResourcesByHandleId.get(handleId)?.deref();
  if (resource === undefined) {
    nativeResourcesByHandleId.delete(handleId);
    throw tuiError("disposed-handle", "framework handle has no live native resource", { id: handleId });
  }
  return resource as T;
}

/** Retrieves the raw native resource for a live framework value. */
export function nativeResourceOf<T extends object>(handle: object): T {
  const resource = nativeResources.get(handle);
  if (resource === undefined) throw tuiError("disposed-handle", "framework value has no live native resource");
  return resource as T;
}

/** Removes a value's raw native association after native disposal. */
export function releaseNativeResource(handle: object, handleId?: HandleId): void {
  nativeResources.delete(handle);
  if (handleId !== undefined) nativeResourcesByHandleId.delete(handleId);
}

/** Disposes the raw native resource owned by a framework value. */
export function disposeNativeResource(handle: object): void {
  nativeResourceOf<DisposableResource>(handle).dispose();
}
