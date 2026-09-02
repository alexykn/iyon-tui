import { describe, expect, test } from "bun:test";

import { History, TextStreamSource, Tui, View } from "../src/index.ts";

describe("T5 native TUI handles", () => {
  test("synchronous native mutations do not allocate Promise wrappers", () => {
    const source = TextStreamSource.create();
    expect(source.replace("direct")).toMatchObject({ revision: 1n });
    expect(source.snapshot()).toMatchObject({ text: "direct", revision: 1n });
    const history = new History();
    expect(history.push(View.text("direct"))).toBeGreaterThanOrEqual(0);
    expect(history.layout()).toEqual({ padding: 0, gap: 0 });
    source.dispose();
    history.dispose();
  });

  test("keeps Tui-created TextInput state in one native object", async () => {
    const tui = await Tui.open({ headless: true });
    const input = tui.createTextInput();
    await input.setText("hello 🌍");
    expect(await input.text()).toBe("hello 🌍");
    expect(await input.cursorBytes()).toBe(Buffer.byteLength("hello 🌍"));
    await input.setMultiline(true);
    expect(await input.isMultiline()).toBe(true);
    await input.clear();
    expect(await input.text()).toBe("");
    input.dispose();
    try { input.text(); throw new Error("expected disposed handle error"); }
    catch (error) { expect(error).toMatchObject({ category: "disposed-handle" }); }
    await tui.close();
  });

  test("preserves Source revision and sealed-state errors", () => {
    const source = TextStreamSource.create();
    source.append("first");
    expect(source.snapshot()).toMatchObject({ text: "first", revision: 1n, sealed: false });
    source.seal();
    expect(source.snapshot().sealed).toBe(true);
    expect(() => source.append("late")).toThrow(/sealed/);
    source.dispose();
  });
  test("history accepts a retained view and component handles are shared", async () => {
    const tui = await Tui.open({ headless: true });
    const history = new History();
    await history.push(View.text("history"));
    const slot = tui.createViewSlot(View.spacer(0));
    const first = await slot.view();
    const second = await slot.view();
    expect(first).not.toBe(second);
    expect(await slot.revision()).toBe(0);
    slot.dispose();
    history.dispose();
    await tui.close();
  });
});
