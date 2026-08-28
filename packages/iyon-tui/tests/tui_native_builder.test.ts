import { describe, expect, test } from "bun:test";

import { native } from "../src/native.ts";
import {
  nativeViewAbiSession,
  nativeViewRefForNodeId,
  releaseNativeViewRef,
  tryNativeAxisCreateRender,
} from "../src/native_view_abi.ts";
import { AppHarness } from "../src/testing/index.ts";
import { nodeForBridge } from "../src/view-internals.ts";
import { View } from "../src/api/view/view.ts";

type HostContract = {
  render(view: object): void;
  screenRows(): string[];
  tuiViewAbiHostPointer(): number;
  dispose(): void;
};

const Host = native.NativeTuiHost as unknown as (new (width: number, height: number, headless: boolean) => HostContract) | undefined;

describe("PERF-11.8 native builders and small constructors", () => {
  test("constructs small axes through generated scalar constructors", () => {
    if (Host === undefined || nativeViewAbiSession() === undefined) return;
    const host = new Host(8, 4, true);
    const oracle = new Host(8, 4, true);
    const left = View.spacer(1);
    const right = View.spacer(2);
    const next = View.vertical((column) => {
      column.fixed(1, left);
      column.flex(right);
    });
    try {
      host.render(nodeForBridge(left));
      const leftRef = nativeViewRefForNodeId(left);
      host.render(nodeForBridge(right));
      const rightRef = nativeViewRefForNodeId(right);
      expect(leftRef).toBeGreaterThan(0);
      expect(rightRef).toBeGreaterThan(0);
      const result = tryNativeAxisCreateRender(host, next, false, 0, [
        { view: left, trackWord: 3 | (1 << 8) },
        { view: right, trackWord: 4 | (1 << 8) },
      ]);
      expect(result).toBeGreaterThan(0);
      releaseNativeViewRef(nativeViewAbiSession(), leftRef!);
      releaseNativeViewRef(nativeViewAbiSession(), rightRef!);
      oracle.render(nodeForBridge(next));
      expect(host.screenRows()).toEqual(oracle.screenRows());
      releaseNativeViewRef(nativeViewAbiSession(), result!);
    } finally {
      host.dispose();
      oracle.dispose();
    }
  });

  test("constructs a compact cold axis through the native builder and keeps text on fallback", async () => {
    if (Host === undefined || nativeViewAbiSession() === undefined) return;
    const tui = await AppHarness.open({ width: 8, height: 6 });
    const oracle = new Host(8, 6, true);
    const spacers = Array.from({ length: 6 }, (_, index) => View.spacer(index + 1));
    const next = View.vertical(spacers);
    try {
      tui.render({ body: next });
      oracle.render(nodeForBridge(next));
      expect(tui.screenRows()).toEqual(oracle.screenRows());

      const textFallback = View.vertical([View.text("unsupported"), View.spacer(1)]);
      tui.render({ body: textFallback });
      oracle.render(nodeForBridge(textFallback));
      expect(tui.screenRows()).toEqual(oracle.screenRows());
    } finally {
      tui.close();
      oracle.dispose();
    }
  });
});
