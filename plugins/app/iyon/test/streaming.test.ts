import { describe, expect, test } from "bun:test";
import { installIyonVirtualModules } from "@iyon/runtime";
import { Scene, Tui, View } from "@iyon/tui";
import { AssistantStreamBuffer, NativeAssistantStream } from "../src/streaming.ts";

installIyonVirtualModules();

describe("assistant streaming", () => {
  test("coalesces adjacent semantic segments and seals", () => {
    const stream = new AssistantStreamBuffer(); stream.append("text", "a"); stream.append("text", "b"); stream.append("thinking", "c");
    expect(stream.snapshot()).toEqual([{ kind: "text", text: "ab" }, { kind: "thinking", text: "c" }]);
    stream.seal(); expect(stream.isSealed()).toBe(true); expect(() => stream.append("text", "d")).toThrow();
  });
  test("uses the native stream handle", async () => {
    const stream = new NativeAssistantStream(); await stream.append("text", "hello");
    expect((await stream.snapshot()).text).toBe("hello"); await stream.seal(); await stream.dispose();
  });

  test("keeps an open reference definition out of the resident prefix", async () => {
    const tui = await Tui.open({ width: 12, height: 5, headless: true });
    const history = tui.createHistory();
    const stream = new NativeAssistantStream();
    const chunks = [
      ["[foo", 4], ["]:", 4], [" htt", 16], ["ps", 4], ["://e", 0], ["xa", 1],
      ["mple", 16], [".c", 32], ["om\\n\\n", 4], ["Ea", 1], ["rlie", 4], ["r ", 16],
      ["para", 4], ["gr", 4], ["aph.", 16], ["\\n\\n", 4], ["Late", 4], ["r ", 1],
      ["[foo", 32], ["] ", 1], ["and ", 1], ["![", 16], ["x](h", 0], ["tt", 4],
      ["ps:/", 16], ["/x", 16], [".tes", 32], ["t/", 4], ["a.pn", 4], ["g)", 0],
      [".\\n\\nt", 32], ["ai", 0], ["l-3\\n", 0], ["\\n", 0],
    ] as const;
    try {
      await tui.render(new Scene(View.spacer(0), history));
      await history.pushStream(stream.native);
      await stream.append("thinking", "reasoning 3");
      for (const [chunk, delay] of chunks) {
        await stream.append("text", chunk);
        tui.advance(delay);
      }
      for (let tick = 0; tick < 40; tick += 1) tui.advance(16);
      await stream.seal();
      await history.sealStream(stream.native);
      expect((await stream.snapshot()).sealed).toBe(true);
    } finally {
      stream.dispose();
      await tui.close();
    }
  });
});
