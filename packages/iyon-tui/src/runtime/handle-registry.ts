import { asTuiError, tuiError } from "../api/errors.ts";
import type { HandleId } from "../api/controls/framework-handle.ts";
import {
  disposeNativeResource,
  registerNativeResource,
  releaseNativeResource,
} from "../transport/native/resources.ts";

const handleIds = new WeakMap<object, HandleId>();
let nextHandleId = 1;

/** Registers runtime-owned identity and delegates raw storage to transport. */
export function registerFrameworkHandle(handle: object, resource: object): HandleId {
  if (nextHandleId > Number.MAX_SAFE_INTEGER) throw new Error("TUI framework handle identity exhausted");
  const id = nextHandleId++ as HandleId;
  try {
    handleIds.set(handle, id);
    registerNativeResource(handle, resource, id);
    return id;
  } catch (error) {
    handleIds.delete(handle);
    throw error;
  }
}

/** Resolves the local identity owned by the live runtime handle. */
export function handleIdOf(handle: object): HandleId {
  const id = handleIds.get(handle);
  if (id === undefined) throw tuiError("disposed-handle", "framework handle has no local identity");
  return id;
}

/** Removes runtime identity and its associated raw resource. */
export function releaseFrameworkHandle(handle: object): void {
  const id = handleIds.get(handle);
  releaseNativeResource(handle, id);
  handleIds.delete(handle);
}

/** Shared lifecycle implementation for nominal framework-owned handles. */
export function disposeFrameworkResource(handle: object): void {
  try {
    disposeNativeResource(handle);
  } catch (error) {
    throw asTuiError(error);
  } finally {
    releaseFrameworkHandle(handle);
  }
}
