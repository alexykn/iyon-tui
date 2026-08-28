import { tuiError } from "./api/errors.ts";
import {
  handleIdOf,
  nativeResourceOf,
  registerNativeResource,
} from "./handle-registry.ts";
import type { ComponentId } from "./types.ts";

export { handleIdOf, nativeResourceOf, registerNativeResource } from "./handle-registry.ts";

/** Reads an optional native component identity without consulting public methods. */
export function nativeComponentIdOf(handle: object): ComponentId | undefined {
  const resource = nativeResourceOf<{ componentId?: () => number | null }>(handle);
  const id = resource.componentId?.();
  return id === null || id === undefined ? undefined : id as ComponentId;
}

/** Resolves the native identity used by a component View node. */
export function componentIdOf(handle: object): ComponentId {
  const id = nativeComponentIdOf(handle);
  if (id === undefined) {
    throw tuiError("invalid-handle", "framework component has no native component identity");
  }
  return id;
}

export function requireNativeClass<T>(factory: T | undefined, name: string): T {
  if (factory === undefined) throw tuiError("runtime", `${name} is unavailable in the native addon`);
  return factory;
}
