import { describe, expect, test } from "bun:test";
import { mkdir, mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { installIyonVirtualModules } from "../../../../packages/iyon-runtime/src/virtual-modules.ts";
import { nodeForBridge } from "../../../../packages/iyon-tui/src/values/view.ts";
installIyonVirtualModules();
const { lsTool } = await import("../src/execute.ts");
const context = (root: string) => ({ workspace: { root }, signal: new AbortController().signal } as never);

describe("ls tool", () => {
  test("compiles lifecycle and multiline result fixtures through native views", () => {
    for (const state of ["preparing", "prepared", "running"] as const) {
      expect(nodeForBridge(lsTool.renderCall({ id: "call" as never, name: "ls", arguments: { path: "." }, state }) as never)).toBeDefined();
    }
    expect(nodeForBridge(lsTool.renderResult({ content: [{ type: "text", text: "a.txt\nb.txt" }], details: {}, isError: false }) as never)).toBeDefined();
    expect(nodeForBridge(lsTool.renderResult({ content: [{ type: "text", text: "ls failed\nagain" }], details: {}, isError: true }) as never)).toBeDefined();
  });

  test("sorts entries, includes dotfiles, and marks directories", async () => {
    const root = await mkdtemp(join(tmpdir(), "iyon-ls-"));
    await mkdir(join(root, "Dir")); await writeFile(join(root, "a.txt"), "a"); await writeFile(join(root, ".hidden"), "h");
    const result = await lsTool.execute(context(root), {});
    expect(result.content[0]).toMatchObject({ type: "text", text: ".hidden\na.txt\nDir/" });
  });

  test("reports empty directories", async () => {
    const root = await mkdtemp(join(tmpdir(), "iyon-ls-empty-"));
    const result = await lsTool.execute(context(root), {});
    expect(result.content[0]).toMatchObject({ text: "(empty directory)" });
  });
});
