import { describe, expect, test } from "bun:test";
import { defineTool } from "@iyon/sdk";
import { installIyonVirtualModules } from "../../src/virtual-modules.ts";
import { View } from "../../../iyon-tui/src/index.ts";

installIyonVirtualModules();

describe("tool contribution contract", () => {
  test("keeps execution and both presentation methods together", () => {
    const tool = defineTool({
      name: "weather",
      description: "Read the weather",
      inputSchema: { type: "object" },
      execute: async () => ({ content: [{ type: "text", text: "sunny" }], details: {}, isError: false }),
      renderCall: () => View.text("weather call") as never,
      renderResult: () => View.text("weather result") as never,
    });

    expect(tool.modelSpec).toEqual({ name: "weather", description: "Read the weather", inputSchema: { type: "object" } });
    expect(tool.execute).toBeFunction();
    expect(tool.renderCall).toBeFunction();
    expect(tool.renderResult).toBeFunction();
  });

  test("rejects a contribution without presentation semantics", () => {
    expect(() => defineTool({
      name: "broken",
      description: "broken",
      inputSchema: {},
      execute: async () => ({ content: [], details: {}, isError: false }),
      renderCall: undefined as never,
      renderResult: undefined as never,
    })).toThrow("renderCall and renderResult");
  });
});
