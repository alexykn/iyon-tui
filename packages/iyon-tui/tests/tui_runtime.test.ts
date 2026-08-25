import { describe, expect, test } from "bun:test";

import { Scene, Tui, View } from "../src/index.ts";

describe("native interaction host", () => {
  test("local editing stays native and submit crosses as an action", async () => {
    const tui = await Tui.open({ width: 20, height: 4, headless: true });
    const input = tui.createTextInput({ multiline: true, border: { style: "plain", edges: "topBottom", color: "white" } });
    await tui.render(new Scene(View.component(input)));

    tui.enqueue({ type: "key", key: "a" });
    expect(await input.text()).toBe("a");

    tui.route(await input.submitted(), "submit");
    tui.enqueue({ type: "key", key: "Enter" });
    await expect(tui.nextAction()).resolves.toEqual({ actionId: "submit", payload: "a" });
    await tui.close();
  });

  test("cancellation rejects pending native action wait and close is idempotent", async () => {
    const tui = await Tui.open({ headless: true });
    const controller = new AbortController();
    const waiting = tui.nextAction(controller.signal);
    controller.abort();
    await expect(waiting).rejects.toMatchObject({ category: "cancelled" });
    await tui.close();
    await tui.close();
  });
});
