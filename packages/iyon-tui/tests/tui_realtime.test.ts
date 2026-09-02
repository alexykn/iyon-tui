import { describe, expect, test } from "bun:test";
import { Scene, View } from "../src/index.ts";
import { AppHarness } from "../src/testing/index.ts";

function sleep(milliseconds: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

async function closeAfter(tui: { close(): void }, milliseconds: number): Promise<void> {
  await sleep(milliseconds);
  tui.close();
}

describe("real-time native TUI driving", () => {
  test("real_runtime_drives_slot_tick_without_manual_clock_advance", async () => {
    const tui = await AppHarness.open({ width: 60, height: 12 });
    try {
      const slot = tui.createViewSlot(View.text("frame one"));
      await slot.setAnimation([View.text("frame one"), View.text("frame two")], 80);
      await tui.render(new Scene(slot.view().fillWidth()));
      const before = tui.screenRows();
      const nextEvent = tui.nextEvent();
      const closer = closeAfter(tui, 160);
      await sleep(120);
      const after = tui.screenRows();
      await closer;
      await nextEvent;

      expect(after).not.toEqual(before);
    } finally {
      await tui.close();
    }
  });

});
