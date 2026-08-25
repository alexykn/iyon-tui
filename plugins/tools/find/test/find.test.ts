import { describe, expect, test } from "bun:test";
import { mkdir, mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { installIyonVirtualModules } from "../../../../packages/iyon-runtime/src/virtual-modules.ts";
import { nodeForBridge } from "../../../../packages/iyon-tui/src/values/view.ts";
installIyonVirtualModules();
const { findTool } = await import("../src/execute.ts");
const context = (root: string) => ({ workspace: { root }, cwd: root, signal: new AbortController().signal } as never);

describe("find tool", () => {
  test("compiles lifecycle and multiline result fixtures through native views", () => {
    for (const state of ["preparing", "prepared", "running"] as const) {
      expect(nodeForBridge(findTool.renderCall({ id: "call" as never, name: "find", arguments: { pattern: "*.ts", path: "." }, state }) as never)).toBeDefined();
    }
    expect(nodeForBridge(findTool.renderResult({ content: [{ type: "text", text: "src/a.ts\nsrc/b.ts" }], details: {}, isError: false }) as never)).toBeDefined();
    expect(nodeForBridge(findTool.renderResult({ content: [{ type: "text", text: "find failed\nagain" }], details: {}, isError: true }) as never)).toBeDefined();
  });

  test("returns relative POSIX paths and excludes node_modules", async () => {
    const root = await mkdtemp(join(tmpdir(), "iyon-find-"));
    await mkdir(join(root, "src")); await mkdir(join(root, "node_modules"));
    await writeFile(join(root, "src/a.ts"), "a"); await writeFile(join(root, "node_modules/b.ts"), "b");
    const result = await findTool.execute(context(root), { pattern: "**/*.ts" });
    expect(result.content[0]).toMatchObject({ type: "text", text: "src/a.ts" });
    expect(result.content[0]).not.toMatchObject({ text: expect.stringContaining("node_modules") });
  });

  test("handles no matches without an error result", async () => {
    const root = await mkdtemp(join(tmpdir(), "iyon-find-empty-"));
    const result = await findTool.execute(context(root), { pattern: "*.missing" });
    expect(result).toMatchObject({ isError: false, content: [{ text: "No files found matching pattern" }] });
  });
});
