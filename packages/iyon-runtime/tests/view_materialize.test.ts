import { describe, expect, test } from "bun:test";
import type { Pointer } from "bun:ffi";

import { native } from "../src/native.ts";
import { nativeViewAbiSession } from "../src/tui/native_view_abi.ts";
import { hostRenderRef, viewRefForNodeId, viewReleaseMany } from "../src/tui/generated/view_calls.ts";
import {
  decodeMaterializeStatus,
  materializeSpacer,
  SPACER_STATUS_DETAIL,
  type BridgeSpacerMaterializeNode,
} from "../src/tui/generated/view_materialize.ts";
import { nodeForBridge, nodeIdPair, View } from "../src/tui/values/view.ts";
import { MaterializeTx } from "../src/tui/retained_dag.ts";

type AbiHost = {
  render(view: object): void;
  tuiViewAbiHostPointer(): number;
  screenRows(): string[];
  dispose(): void;
};

const Host = native.NativeTuiHost as unknown as
  (new (width: number, height: number, headless: boolean) => AbiHost) | undefined;

/**
 * PERF-12 T5 vertical-slice conformance: the generator emits a semantic
 * materializer end-to-end and the minted NativeRef participates in the
 * shared semantic cache exactly like hand-written constructor callers.
 */
describe("PERF-12 T5 generated materializer vertical slice", () => {
  test("declares the §74 status detail kind", () => {
    expect(SPACER_STATUS_DETAIL).toBe("none");
  });

  test("materializes a spacer through the generated FFI path", () => {
    const session = nativeViewAbiSession();
    if (session === undefined) return;
    const view = View.spacer(3);
    const node = nodeForBridge(view) as unknown as BridgeSpacerMaterializeNode;
    const tx = new MaterializeTx(session.symbols, session.runtime, session.abi.generation, 0);
    const reference = materializeSpacer(node, tx);
    expect(reference).toBeGreaterThan(0);
    const decoded = decodeMaterializeStatus(reference);
    expect(decoded.ok).toBe(true);
    expect(decoded.reference).toBe(reference);
    // The u32 halves must match the shared NodeId split convention.
    const [lowExpected, highExpected] = nodeIdPair(view);
    expect(lowExpected).toBe(node.id >>> 0);
    expect(highExpected).toBe(Math.floor(node.id / 0x1_0000_0000));
    // §23 semantic cache first: consulting the NodeId resolves the same
    // NativeRef the materializer just published.
    const [low, high] = nodeIdPair(view);
    const consulted = viewRefForNodeId(session.symbols, session.runtime, low, high);
    expect(consulted).toBe(reference);
    viewReleaseMany(session.symbols, session.runtime, new Uint32Array([reference]), 1);
    tx.releaseAll();
  });

  test("renders the materialized spacer through a real host", () => {
    if (Host === undefined) return;
    const session = nativeViewAbiSession();
    if (session === undefined) return;
    const host = new Host(8, 4, true);
    try {
      host.render(nodeForBridge(View.spacer(4)));
      const view = View.spacer(2);
      const tx = new MaterializeTx(session.symbols, session.runtime, session.abi.generation, 0);
      const reference = materializeSpacer(nodeForBridge(view) as unknown as BridgeSpacerMaterializeNode, tx);
      expect(hostRenderRef(
        session.symbols,
        session.runtime,
        host.tuiViewAbiHostPointer() as unknown as Pointer,
        reference,
      )).toBe(0);
      expect(host.screenRows()).toEqual(["        ", "        ", "        ", "        "]);
      viewReleaseMany(session.symbols, session.runtime, new Uint32Array([reference]), 1);
      tx.releaseAll();
    } finally {
      host.dispose();
    }
  });
});
