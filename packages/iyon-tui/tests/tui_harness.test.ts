import { describe, expect, test } from "bun:test";

import { AppHarness, TextStream, View } from "../src/index.ts";

describe("native headless harness", () => {
  test("uses native snapshots and dispatches input through the mounted host", async () => {
    const harness = await AppHarness.open({ width: 20, height: 4 });
    const input = harness.createTextInput({ multiline: true, border: { style: "plain", edges: "topBottom", color: "white" } });
    const history = harness.createHistory();
    await history.push(View.text("native history"));
    await harness.render({ body: View.vertical([View.component(input), View.text("footer")]), history });
    expect(harness.screenRows().at(-1)?.startsWith("footer")).toBe(true);
    expect(harness.nativeHistoryRows().some((row) => row.includes("native history"))).toBe(true);

    harness.bindKey("Escape", "escape");
    harness.pressKey("a");
    expect(await input.text()).toBe("a");
    harness.route(await input.submitted(), "submit");
    harness.pressKey("Enter");
    await expect(harness.nextAction()).resolves.toEqual({ actionId: "submit", payload: "a" });

    harness.advance(25);
    expect(harness.now()).toBe(25);
    harness.close();
    expect(harness.exited()).toBe(true);
  });

  test("streams update the mounted native History", async () => {
    const harness = await AppHarness.open({ width: 24, height: 6 });
    const history = harness.createHistory();
    const stream = new TextStream();
    await harness.render({ body: View.text("footer"), history });
    await history.pushStream(stream);
    await stream.update("assistant");
    expect(harness.screenRows().some((row) => row.includes("assistant"))).toBe(true);
    stream.seal();
    expect(() => stream.update("late")).toThrow();
    harness.close();
  });

  test("animates a generic native view slot", async () => {
    const harness = await AppHarness.open({ width: 80, height: 6 });
    const slot = harness.createViewSlot(View.text("frame one"));
    await slot.setAnimation([View.text("frame one"), View.text("frame two")], 80);
    await harness.render({ body: View.component(slot) });
    expect(harness.screenRows().some((row) => row.includes("frame one"))).toBe(true);
    harness.advance(80);
    expect(harness.screenRows().some((row) => row.includes("frame two"))).toBe(true);
    slot.dispose();
    harness.close();
  });

  test("observes native styles and terminal-cell Unicode positions", async () => {
    const harness = await AppHarness.open({ width: 12, height: 2 });
    await harness.render({ body: View.text("a🌍b").bold().noWrap() });
    expect(harness.cellXOfText(1, "🌍")).toBe(1);
    expect(harness.cellXOfText(1, "b")).toBe(3);
    expect(harness.styleAt(1, 0).bold).toBe(true);
    harness.close();
    expect(harness.exited()).toBe(true);
  });
});
