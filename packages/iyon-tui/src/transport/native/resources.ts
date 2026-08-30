import { tuiError } from "../../api/errors.ts";
import type { HandleId } from "../../api/controls/framework-handle.ts";
import {
  runtimeResourceRegistry,
  type NativeResourceKind,
} from "./resource-registry.ts";

interface DisposableResource {
  dispose(): void;
}

/**
 * Handle-local lookup retained as a cheap identity check. The environment
 * registry below is authoritative for HandleId resolution and leases.
 */
const nativeResources = new WeakMap<object, object>();

/** Registers one resource in the shared environment resolver. */
export function registerNativeResource(
  handle: object,
  resource: object,
  handleId?: HandleId,
  kind: NativeResourceKind = "framework",
): void {
  if (handleId !== undefined) {
    runtimeResourceRegistry().register({ handle, resource, handleId, kind });
  }
  // Some opaque control values, such as Output channels, intentionally have
  // no semantic HandleId. They still use the same handle-local native map;
  // only attachment-capable framework handles enter the shared resolver.
  nativeResources.set(handle, resource);
}

/** Retrieves a raw native resource through its semantic framework identity. */
export function nativeResourceForHandleId<T extends object>(handleId: HandleId): T {
  return runtimeResourceRegistry().resourceForHandleId(handleId) as T;
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
  if (handleId !== undefined) runtimeResourceRegistry().release(handleId);
}

/** Disposes the raw native resource owned by a framework value. */
export function disposeNativeResource(handle: object): void {
  nativeResourceOf<DisposableResource>(handle).dispose();
}
