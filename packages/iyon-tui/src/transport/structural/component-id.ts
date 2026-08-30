import { tuiError } from "../../api/errors.ts";
import { nativeResourceForHandleId } from "../native/resources.ts";
import type { HandleId } from "../../api/controls/framework-handle.ts";
import type { ComponentId } from "../../api/extensions/traits/component.ts";

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

/** Resolves the semantic HandleId to the current physical component identity. */
export function componentIdForHandleId(handleId: HandleId): ComponentId {
  return componentIdOfResource(nativeResourceForHandleId<NativeComponentResource>(handleId, "component"));
}
