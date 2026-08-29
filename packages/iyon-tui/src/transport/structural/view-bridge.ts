import type { View } from "../../api/view/view.ts";
import type { BridgeViewNode } from "./ir.ts";
import { lowerColdView } from "./cold-lowering.ts";

/**
 * Transitional bridge access for H3-B consumers. The semantic node is the
 * authoritative View representation; this function derives the complete
 * bridge object only at the legacy structural boundary.
 */
export function nodeForBridge(view: View): BridgeViewNode {
  return lowerColdView(view);
}
