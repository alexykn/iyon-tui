import { tuiError } from "../../api/errors.ts";
import { nativeResourceOf } from "../native/resources.ts";
import { BRIDGE_VIEW_KIND } from "./ir.ts";
import type { ComponentHandle, ComponentId } from "../../types.ts";
import { View } from "../../api/view/view.ts";

/** Reads an optional native component identity without consulting public methods. */
function nativeComponentIdOf(handle: object): ComponentId | undefined {
  const resource = nativeResourceOf<{ componentId?: () => number | null }>(handle);
  const id = resource.componentId?.();
  return id === null || id === undefined ? undefined : id as ComponentId;
}

/** Resolves the native identity used by a component View node. */
function componentIdOf(handle: object): ComponentId {
  const id = nativeComponentIdOf(handle);
  if (id === undefined) {
    throw tuiError("invalid-handle", "framework component has no native component identity");
  }
  return id;
}

/**
 * Private component-placement lowering. Public controls expose `view()`;
 * ordinary consumers do not need to construct native component nodes or know
 * their identity representation. Retained callers enter through compose.ts,
 * which keeps the existing semantic slot and View identity behavior.
 */
export function componentIdForPlacement(handle: ComponentHandle): ComponentId {
  return componentIdOf(handle);
}

export function componentViewFor(handle: ComponentHandle, componentId = componentIdForPlacement(handle)): View {
  // The constructor is intentionally private in the public declaration. This
  // module-local bridge is the sole friend path for control projections.
  const Constructor = View as unknown as new (node: object) => View;
  return new Constructor({ kind: BRIDGE_VIEW_KIND.component, handle: componentId });
}
