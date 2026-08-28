import { describe, expect, test } from "bun:test";

import { Scene, View } from "../src/index.ts";
import { AppHarness } from "../src/testing/index.ts";

describe("native interaction host", () => {
  test("local editing stays native and submit crosses as an action", async () => {
    const tui = await AppHarness.open({ width: 20, height: 4 });
    const input = tui.createTextInput({ multiline: true, border: { style: "plain", edges: "topBottom", color: { type: "named", value: "white" } } });
    await tui.render(new Scene(input.view()));

    tui.pressKey("a");
    expect(await input.text()).toBe("a");

    const submitted = await input.submitted();
    expect("payload" in submitted).toBe(false);
    tui.route(submitted, "submit");
    tui.pressKey("Enter");
    await expect(tui.nextEvent()).resolves.toEqual({ type: "output", routeId: "submit", payload: "a" });
    await tui.close();
  });

  test("cancellation rejects pending native action wait and close is idempotent", async () => {
    const tui = await AppHarness.open();
    const controller = new AbortController();
    const waiting = tui.nextEvent(controller.signal);
    controller.abort();
    await expect(waiting).rejects.toMatchObject({ category: "cancelled" });
    await tui.close();
    await tui.close();
  });
});
