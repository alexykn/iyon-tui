import { describe, expect, test } from "bun:test";

import {
  ComponentAdapterBridge,
  OutputRouter,
  RendererAdapter,
  TextContent,
  TextRewriterAdapter,
  View,
} from "../src/index.ts";

describe("T5 JS-thread trait adapters", () => {
  test("renderer and rewriter callbacks run as owned JS promises", async () => {
    const calls: string[] = [];
    const renderer = new RendererAdapter({
      render(view) { calls.push("render"); return view.bold(); },
    });
    const result = await renderer.render(View.text("x"));
    const rewriter = new TextRewriterAdapter({
      rewrite(content) { calls.push("rewrite"); return content.rewrite((text) => text.toUpperCase()); },
    });
    expect(await rewriter.rewrite(TextContent.plain("x"))).toMatchObject({ kind: "text-content" });
    expect(result).toBeInstanceOf(View);
    expect(calls).toEqual(["render", "rewrite"]);
  });

  test("output routing is FIFO and rejects conflicts", () => {
    const router = new OutputRouter<number>();
    router.route("change", (value) => Number(value.value));
    expect(() => router.route("change", () => 2)).toThrow(/route/);
    router.emit("change", { value: 1 });
    router.emit("change", { value: 2 });
    expect(router.drain()).toEqual([1, 2]);
  });

  test("component callbacks receive no borrowed native value", async () => {
    const bridge = new ComponentAdapterBridge({
      view: async () => View.text("component"),
      onKey: async (event) => ({ type: event.key === "Enter" ? "handled" : "ignored" }),
    });
    const context = { componentId: 1 as never, emit: () => {} };
    expect((await bridge.view(context)).kind).toBe("view");
    expect(await bridge.key({ type: "key", key: "Enter" }, context)).toEqual({ type: "handled" });
  });
});
