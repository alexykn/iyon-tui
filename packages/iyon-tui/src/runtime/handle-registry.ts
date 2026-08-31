import { asTuiError, tuiError } from "../api/errors.ts";
import type { HandleId } from "../api/controls/framework-handle.ts";
import {
  disposeNativeResource,
  registerNativeResource,
  releaseNativeResource,
} from "../transport/native/resources.ts";
import { runtimeResourceRegistry } from "../transport/native/resource-registry.ts";
import type {
  NativeResourceKind,
  ResourceOwner,
} from "./native-resource-registry.ts";

export interface FrameworkHandleResourceOptions {
  readonly owner?: ResourceOwner;
  readonly acceptedNodeKinds?: ReadonlySet<number>;
}

const handleIds = new WeakMap<object, HandleId>();
const HANDLE_ID_COUNTER = Symbol.for("iyon:tui:private-handle-counter");
type HandleGlobals = typeof globalThis & { [HANDLE_ID_COUNTER]?: { next: number } };
const handleGlobals = globalThis as HandleGlobals;
const handleIdCounter = handleGlobals[HANDLE_ID_COUNTER] ??= { next: 1 };

/** Registers runtime-owned identity and delegates raw storage to transport. */
export function registerFrameworkHandle(
  handle: object,
  resource: object,
  kind: NativeResourceKind = "framework",
  options: FrameworkHandleResourceOptions = {},
): HandleId {
  if (handleIdCounter.next > Number.MAX_SAFE_INTEGER) throw new Error("TUI framework handle identity exhausted");
  const id = handleIdCounter.next++ as HandleId;
  const resourceKind = kind === "component" || kind === "text-input" ? "component" : kind;
  try {
    handleIds.set(handle, id);
    registerNativeResource(
      handle,
      resource,
      id,
      resourceKind,
      options.owner,
      options.acceptedNodeKinds,
    );
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
  const id = handleIdOf(handle);
  runtimeResourceRegistry().beginDisposal(id);
  try {
    disposeNativeResource(handle);
    releaseFrameworkHandle(handle);
  } catch (error) {
    // Native disposal can reject a mounted/in-use resource after the shared
    // registry has tentatively blocked new prepares. Restore liveness when no
    // native release completed so a later post-unmount dispose can succeed.
    runtimeResourceRegistry().cancelDisposal(id);
    throw asTuiError(error);
  }
}
