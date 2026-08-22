import { describe, expect, test } from "bun:test";

import { native } from "../src/native.ts";
import { AppHarness, StyleSpec, Tui, View, TextSpan } from "../src/tui/index.ts";
import { nodeForBridge } from "../src/tui/values/view.ts";
import { nativeViewAbiSession } from "../src/tui/native_view_abi.ts";

type HostContract = {
  tuiViewAbiHostPointer(): number;
  render(view: object): void;
  screenRows(): string[];
  dispose(): void;
};

const Host = native.NativeTuiHost as unknown as (new (width: number, height: number, headless: boolean) => HostContract) | undefined;

describe("PERF-11.9 native strings and style atoms", () => {
  test("preserves Unicode, styled spans, and embedded NUL parity", async () => {
    if (Host === undefined || nativeViewAbiSession() === undefined) return;
    const tui = await Tui.open({ width: 24, height: 4, headless: true });
    const oracle = new Host(24, 4, true);
    const values = [
      View.text("héllo 🌍").bold().foreground("red").noWrap(),
      View.styledText([
        TextSpan.plain("one"),
        TextSpan.styled(" ✓", new StyleSpec().italic()),
        TextSpan.styled(" three", new StyleSpec().underline()),
      ]),
      View.text("left\0right"),
      View.text("lone\ud800surrogate"),
      View.styledText([
        TextSpan.styled("left\0", new StyleSpec().bold()),
        TextSpan.styled("right\0✓", new StyleSpec().italic()),
      ]),
    ];
    try {
      // PERF-12 T4 note: the old pre-materialization via the native
      // single-text builder route is gone with the pending backing; every
      // value now renders through the production router and must still match
      // the Direct oracle exactly.
      for (const value of values) {
        tui.render({ body: value });
        oracle.render(nodeForBridge(value));
        expect(tui.screenRows()).toEqual(oracle.screenRows());
      }
    } finally {
      tui.close();
      oracle.dispose();
    }
  });

  test("retains native style atoms through the headless style surface", async () => {
    if (nativeViewAbiSession() === undefined) return;
    const harness = await AppHarness.open({ width: 16, height: 2 });
    try {
      await harness.render({ body: View.text("a🌍b").bold().foreground("cyan").noWrap() });
      expect(harness.cellXOfText(1, "🌍")).toBe(1);
      expect(harness.cellXOfText(1, "b")).toBe(3);
      expect(harness.styleAt(1, 0).bold).toBe(true);
      expect(harness.styleAt(1, 0).foreground).not.toBeNull();
    } finally {
      harness.close();
    }
  });
});
