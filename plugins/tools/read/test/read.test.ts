import { describe, expect, test } from "bun:test";
import { mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { installIyonVirtualModules } from "../../../../packages/iyon-runtime/src/virtual-modules.ts";
import { nodeForBridge } from "../../../../packages/iyon-tui/src/values/view.ts";

installIyonVirtualModules();
const { readTool } = await import("../src/execute.ts");
const context = (root: string) => ({ workspace: { root }, signal: new AbortController().signal } as never);

describe("read tool", () => {
  test("compiles lifecycle and multiline result fixtures through native views", () => {
    for (const state of ["preparing", "prepared", "running"] as const) {
      expect(nodeForBridge(readTool.renderCall({ id: "call" as never, name: "read", arguments: { path: "file.txt", offset: 2, limit: 4 }, state }) as never)).toBeDefined();
    }
    expect(nodeForBridge(readTool.renderResult({ content: [{ type: "text", text: "one\ntwo" }], details: {}, isError: false }) as never)).toBeDefined();
    expect(nodeForBridge(readTool.renderResult({ content: [{ type: "text", text: "read failed\nagain" }], details: {}, isError: true }) as never)).toBeDefined();
  });

  test("reads UTF-8 files and keeps the path in details", async () => {
    const root = await mkdtemp(join(tmpdir(), "iyon-read-"));
    await writeFile(join(root, "file.txt"), "hello\n");
    const result = await readTool.execute(context(root), { path: "file.txt" });
    expect(result).toMatchObject({ content: [{ type: "text", text: "hello\n" }], details: { path: "file.txt" }, isError: false });
  });

  test("exposes contribution-owned call and result views", () => {
    expect(readTool.renderCall({ id: "call" as never, name: "read", arguments: { path: "x" }, state: "running" }).kind).toBe("view");
    expect(readTool.renderResult({ content: [{ type: "text", text: "ok" }], details: {}, isError: false }).kind).toBe("view");
  });
});
