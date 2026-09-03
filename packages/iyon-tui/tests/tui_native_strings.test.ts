import { describe, expect, test } from "bun:test";

import { native } from "../src/transport/native/addon.ts";
import { StyleSpec, View, TextSpan } from "../src/index.ts";
import { AppHarness } from "../src/testing/index.ts";
import { nativeViewAbiSession } from "../src/transport/structural/native-view-abi.ts";
import type { NativeTuiHostContract } from "../src/transport/native/addon.ts";
import { renderRetained } from "./fixtures/native-host.ts";

type HostContract = NativeTuiHostContract;

const Host = native.NativeTuiHost as unknown as (new (width: number, height: number, headless: boolean) => HostContract) | undefined;

describe("PERF-11.9 native strings and style atoms", () => {
  test("preserves Unicode, styled spans, and embedded NUL parity", async () => {
    if (Host === undefined || nativeViewAbiSession() === undefined) return;
    const tui = await AppHarness.open({ width: 24, height: 4 });
    const reference = new Host(24, 4, true);
    const values = [
      View.text("héllo 🌍").bold().foreground({ type: "named", value: "red" }).noWrap(),
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
      // Every value renders through the production semantic router and must
      // match the generated host-ref reference exactly.
      for (const value of values) {
        tui.render({ body: value });
        renderRetained(reference, value);
        expect(tui.screenRows()).toEqual(reference.screenRows());
      }
    } finally {
      tui.close();
      reference.dispose();
    }
  });

  test("retains native style atoms through the headless style surface", async () => {
    if (nativeViewAbiSession() === undefined) return;
    const harness = await AppHarness.open({ width: 16, height: 2 });
    try {
      await harness.render({ body: View.text("a🌍b").bold().foreground({ type: "named", value: "cyan" }).noWrap() });
      expect(harness.cellXOfText(1, "🌍")).toBe(1);
      expect(harness.cellXOfText(1, "b")).toBe(3);
      expect(harness.styleAt(1, 0).bold).toBe(true);
      expect(harness.styleAt(1, 0).foreground).not.toBeNull();
    } finally {
      harness.close();
    }
  });
});
