import { describe, expect, test } from "bun:test";
import { mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { installIyonVirtualModules } from "../../../../packages/iyon-runtime/src/virtual-modules.ts";
import { nodeForBridge } from "../../../../packages/iyon-tui/src/values/view.ts";
installIyonVirtualModules();
const { grepTool } = await import("../src/execute.ts");
const context = (root: string) => ({ workspace: { root }, cwd: root, signal: new AbortController().signal } as never);

describe("grep tool", () => {
  test("compiles lifecycle and multiline result fixtures through native views", () => {
    for (const state of ["preparing", "prepared", "running"] as const) {
      expect(nodeForBridge(grepTool.renderCall({ id: "call" as never, name: "grep", arguments: { pattern: "hello", path: "." }, state }) as never)).toBeDefined();
    }
    expect(nodeForBridge(grepTool.renderResult({ content: [{ type: "text", text: "file.ts:1\nfile.ts:2" }], details: {}, isError: false }) as never)).toBeDefined();
    expect(nodeForBridge(grepTool.renderResult({ content: [{ type: "text", text: "grep failed\nagain" }], details: {}, isError: true }) as never)).toBeDefined();
  });

  test("supports literal and case-insensitive matching", async () => {
    const root = await mkdtemp(join(tmpdir(), "iyon-grep-")); await writeFile(join(root, "file.ts"), "Hello world\nother\n");
    const result = await grepTool.execute(context(root), { pattern: "hello", literal: true, ignoreCase: true, glob: "*.ts" });
    expect(result.content[0]).toMatchObject({ type: "text", text: expect.stringContaining("file.ts:1") });
  });

  test("returns a truthful no-match result", async () => {
    const root = await mkdtemp(join(tmpdir(), "iyon-grep-empty-")); await writeFile(join(root, "file.txt"), "hello\n");
    const result = await grepTool.execute(context(root), { pattern: "missing" });
    expect(result).toMatchObject({ isError: false, content: [{ text: "No matches found" }] });
  });
});
