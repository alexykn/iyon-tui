import { describe, expect, test } from "bun:test";

import { TextContent, View } from "../src/index.ts";
import { ComponentAdapterBridge } from "../src/api/extensions/traits/component.ts";
import { RendererAdapter } from "../src/api/extensions/traits/renderer.ts";
import { TextRewriterAdapter } from "../src/api/extensions/traits/text-rewriter.ts";

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
