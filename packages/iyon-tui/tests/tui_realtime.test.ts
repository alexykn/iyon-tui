import { describe, expect, test } from "bun:test";
import { Scene, TextStream, View } from "../src/index.ts";
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

  test("stream_is_smoothed_in_real_runtime", async () => {
    const tui = await AppHarness.open({ width: 60, height: 12 });
    try {
      const history = tui.createHistory();
      const stream = new TextStream({ projector: "markdown" });
      await tui.render(new Scene(View.spacer(0), history));
      await history.pushStream(stream);
      const text = "abcdefghijklmnopqrstuvwxyz";
      const nextEvent = tui.nextEvent();
      await stream.append(text);
      const first = tui.screenRows().join("\n");
      await sleep(120);
      const second = tui.screenRows().join("\n");
      const closer = closeAfter(tui, 40);
      await closer;
      await nextEvent;

      expect(first).not.toContain(text);
      expect(second).not.toBe(first);
      await stream.dispose();
      await history.dispose();
    } finally {
      await tui.close();
    }
  });

  test("single_stream_append_is_paced_not_atomic", async () => {
    const tui = await AppHarness.open({ width: 60, height: 12 });
    try {
      const history = tui.createHistory();
      const stream = new TextStream({ projector: "markdown" });
      await tui.render(new Scene(View.spacer(0), history));
      await history.pushStream(stream);
      const text = "one streamed update";
      await stream.append(text);
      expect(tui.screenRows().join("\n")).not.toContain(text);
      await sleep(120);
      expect(tui.screenRows().join("\n")).toContain(text[0]!);
      await stream.dispose();
      await history.dispose();
    } finally {
      await tui.close();
    }
  });
});
