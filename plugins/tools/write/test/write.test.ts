import { describe, expect, test } from "bun:test";
import { mkdtemp, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { installIyonVirtualModules } from "../../../../packages/iyon-runtime/src/virtual-modules.ts";
import { nodeForBridge } from "../../../../packages/iyon-tui/src/values/view.ts";
installIyonVirtualModules();
const { writeTool } = await import("../src/execute.ts");
const context = (root: string) => ({ workspace: { root }, signal: new AbortController().signal } as never);

describe("write tool", () => {
  test("compiles lifecycle, multiline, and diff fixtures through native views", () => {
    for (const state of ["preparing", "prepared", "running"] as const) {
      expect(nodeForBridge(writeTool.renderCall({ id: "call" as never, name: "write", arguments: { path: "file.txt", content: "one\ntwo\n" }, state }) as never)).toBeDefined();
    }
    const details = { diff: "@@ -0,0 +1,2 @@\n+one\n+two\n" };
    expect(nodeForBridge(writeTool.renderResult({ content: [{ type: "text", text: "wrote\nfile" }], details, isError: false }) as never)).toBeDefined();
    expect(nodeForBridge(writeTool.renderResult({ content: [{ type: "text", text: "write failed\nagain" }], details, isError: true }) as never)).toBeDefined();
  });

  test("creates parent directories and reports a new-file diff", async () => {
    const root = await mkdtemp(join(tmpdir(), "iyon-write-"));
    const result = await writeTool.execute(context(root), { path: "nested/file.txt", content: "new\n" });
    expect(await readFile(join(root, "nested/file.txt"), "utf8")).toBe("new\n");
    expect(result.details).toMatchObject({ diff: expect.stringContaining("+new") });
  });

  test("does not fabricate a diff for a non-UTF-8 prior file", async () => {
    const root = await mkdtemp(join(tmpdir(), "iyon-write-binary-"));
    await writeFile(join(root, "binary"), new Uint8Array([0xff, 0xfe]));
    const result = await writeTool.execute(context(root), { path: "binary", content: "replacement\n" });
    expect(result.details).toEqual({});
  });
});
