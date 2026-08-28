import type { BridgeViewNode } from "./ir.ts";
import type { View } from "./api/view/view.ts";

/** Private semantic-node association used by the retained materializer. */
const nodes = new WeakMap<View, BridgeViewNode>();

export function setViewNode(view: View, node: BridgeViewNode): void {
  nodes.set(view, node);
}

/** Private bridge access; never re-export from the package entrypoint. */
export function nodeForBridge(view: View): BridgeViewNode {
  const node = nodes.get(view);
  if (node === undefined) throw new TypeError("view is not a runtime semantic value");
  return node;
}
