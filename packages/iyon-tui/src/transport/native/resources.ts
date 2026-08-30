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

function isWeakReferenceable(value: unknown): value is object {
  return (typeof value === "object" && value !== null) || typeof value === "function";
}

/** Registers one resource in the shared environment resolver. */
export function registerNativeResource(
  handle: object,
  resource: object,
  handleId?: HandleId,
  kind: NativeResourceKind = "framework",
  owner?: import("./resource-registry.ts").ResourceOwner,
  acceptedNodeKinds?: ReadonlySet<number>,
): void {
  if (!isWeakReferenceable(handle) || !isWeakReferenceable(resource)) {
    throw tuiError("validation", "native resource registration requires object handles");
  }
  if (nativeResources.has(handle)) {
    throw tuiError("runtime", "framework value already has a native resource");
  }
  if (handleId !== undefined) {
    runtimeResourceRegistry().register({
      handle,
      resource,
      handleId,
      kind,
      owner,
      acceptedNodeKinds,
    });
  }
  // Some opaque control values, such as Output channels, intentionally have
  // no semantic HandleId. They still use the same handle-local native map;
  // only attachment-capable framework handles enter the shared resolver.
  nativeResources.set(handle, resource);
}

/** Retrieves a raw native resource through its semantic framework identity. */
export function nativeResourceForHandleId<T extends object>(
  handleId: HandleId,
  expectedKind?: NativeResourceKind,
): T {
  return runtimeResourceRegistry().resourceForHandleId(handleId, expectedKind) as T;
}

/** Retrieves the raw native resource for a live framework value. */
export function nativeResourceOf<T extends object>(handle: object): T {
  const resource = nativeResources.get(handle);
  if (resource === undefined) throw tuiError("disposed-handle", "framework value has no live native resource");
  const registry = runtimeResourceRegistry();
  if (registry.isRetiredHandle(handle)) {
    throw tuiError("disposed-handle", "framework value has no live native resource");
  }
  const handleId = registry.handleIdFor(handle);
  if (handleId !== undefined) registry.resourceForHandleId(handleId);
  return resource as T;
}

/** Removes a value's raw native association after native disposal. */
export function releaseNativeResource(handle: object, handleId?: HandleId): void {
  if (handleId !== undefined) runtimeResourceRegistry().release(handleId);
  nativeResources.delete(handle);
}

/** Disposes the raw native resource owned by a framework value. */
export function disposeNativeResource(handle: object): void {
  const resource = nativeResources.get(handle);
  if (resource === undefined) throw tuiError("disposed-handle", "framework value has no live native resource");
  (resource as DisposableResource).dispose();
}
