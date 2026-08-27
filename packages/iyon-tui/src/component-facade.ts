import { componentIdOf } from "./handles.ts";
import { BRIDGE_VIEW_KIND } from "./ir.ts";
import type { ComponentHandle } from "./types.ts";
import { View } from "./values/view.ts";

/**
 * Private component-placement lowering. Public controls expose `view()`;
 * ordinary consumers do not need to construct native component nodes or know
 * their identity representation. Retained callers enter through compose.ts,
 * which keeps the existing semantic slot and View identity behavior.
 */
export function componentViewFor(handle: ComponentHandle): View {
  // The constructor is intentionally private in the public declaration. This
  // module-local bridge is the sole friend path for control projections.
  const Constructor = View as unknown as new (node: object) => View;
  return new Constructor({ kind: BRIDGE_VIEW_KIND.component, handle: componentIdOf(handle) });
}
