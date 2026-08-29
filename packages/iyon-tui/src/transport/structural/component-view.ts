import { tuiError } from "../../api/errors.ts";
import { nativeResourceForHandleId, nativeResourceOf } from "../native/resources.ts";
import type { ComponentHandle, HandleId } from "../../api/controls/framework-handle.ts";
import type { ComponentId } from "../../api/extensions/traits/component.ts";
import { componentViewForHandle } from "../../api/view/view.ts";
import type { View } from "../../api/view/view.ts";

interface NativeComponentResource {
  componentId?: () => number | null;
}

/** Reads an optional native component identity without consulting public methods. */
function nativeComponentIdOf(resource: NativeComponentResource): ComponentId | undefined {
  const id = resource.componentId?.();
  return id === null || id === undefined ? undefined : id as ComponentId;
}

function componentIdOfResource(resource: NativeComponentResource): ComponentId {
  const id = nativeComponentIdOf(resource);
  if (id === undefined) {
    throw tuiError("invalid-handle", "framework component has no native component identity");
  }
  return id;
}

/** Resolves the native identity used by a component View node. */
export function componentIdForPlacement(handle: ComponentHandle): ComponentId {
  return componentIdOfResource(nativeResourceOf<NativeComponentResource>(handle));
}

/** Resolves a semantic HandleId at the structural lowering boundary. */
export function componentIdForHandleId(handleId: HandleId): ComponentId {
  return componentIdOfResource(nativeResourceForHandleId<NativeComponentResource>(handleId));
}

/**
 * Transitional compatibility helper retained for H1-era control adapters.
 * Semantic component construction belongs to api/view; the optional native
 * identity argument is intentionally ignored and is not stored in the View.
 */
export function componentViewFor(handle: ComponentHandle, _componentId?: ComponentId): View {
  return componentViewForHandle(handle.id);
}
