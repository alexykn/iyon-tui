import { describe, expect, test } from "bun:test";

import { Component, History, TextStream, Tui, View } from "../src/index.ts";

describe("T5 native TUI handles", () => {
  test("synchronous native mutations do not allocate Promise wrappers", () => {
    const stream = new TextStream();
    expect(stream.update("direct")).toBeUndefined();
    expect(stream.snapshot()).toMatchObject({ text: "direct", revision: 1 });
    const history = new History();
    expect(history.push(View.text("direct"))).toBeGreaterThanOrEqual(0);
    expect(history.layout()).toEqual({ padding: 0, gap: 0 });
    stream.dispose();
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

  test("preserves stream revision and sealed-state errors", async () => {
    const stream = new TextStream();
    await stream.update("first");
    expect(await stream.snapshot()).toEqual({ text: "first", revision: 1, sealed: false });
    stream.seal();
    expect(stream.snapshot().sealed).toBe(true);
    expect(() => stream.update("late")).toThrow(/sealed/);
  });

  test("history accepts a retained view and component handles are shared", async () => {
    const history = new History();
    await history.push(View.text("history"));
    const component = new Component();
    const first = await component.view();
    const second = await component.view();
    expect(first).not.toBe(second);
    expect(await component.revision()).toBe(0);
    component.dispose();
    history.dispose();
  });
});
