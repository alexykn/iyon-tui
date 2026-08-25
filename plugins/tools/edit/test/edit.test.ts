import { describe, expect, test } from "bun:test";
import { mkdtemp, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { installIyonVirtualModules } from "../../../../packages/iyon-runtime/src/virtual-modules.ts";
import { nodeForBridge } from "../../../../packages/iyon-tui/src/values/view.ts";
installIyonVirtualModules();
const { editTool } = await import("../src/execute.ts");
const context = (root: string) => ({ workspace: { root }, signal: new AbortController().signal } as never);

describe("edit tool", () => {
  test("compiles lifecycle, preview, multiline, and diff fixtures through native views", () => {
    for (const state of ["preparing", "prepared", "running"] as const) {
      expect(nodeForBridge(editTool.renderCall({ id: "call" as never, name: "edit", arguments: { path: "file.txt", edits: [{ oldText: "one", newText: "ONE" }] }, state, showArgPreview: true }) as never)).toBeDefined();
    }
    const details = { diff: "@@ -1 +1 @@\n-one\n+ONE\n" };
    expect(nodeForBridge(editTool.renderResult({ content: [{ type: "text", text: "edited\nfile" }], details, isError: false }) as never)).toBeDefined();
    expect(nodeForBridge(editTool.renderResult({ content: [{ type: "text", text: "failed\nedit" }], details, isError: true }) as never)).toBeDefined();
  });

  test("applies disjoint edits against the original and preserves CRLF/BOM", async () => {
    const root = await mkdtemp(join(tmpdir(), "iyon-edit-"));
    await writeFile(join(root, "file.txt"), "\uFEFFone\r\ntwo\r\nthree\r\n");
    const result = await editTool.execute(context(root), { path: "file.txt", edits: [{ oldText: "one", newText: "ONE" }, { oldText: "three", newText: "THREE" }] });
    expect(await readFile(join(root, "file.txt"), "utf8")).toBe("\uFEFFONE\r\ntwo\r\nTHREE\r\n");
    expect(result.details).toMatchObject({ firstChangedLine: 1, diff: expect.stringContaining("-one") });
  });

  test("normalizes legacy payloads and rejects duplicate matches", async () => {
    const root = await mkdtemp(join(tmpdir(), "iyon-edit-legacy-"));
    await writeFile(join(root, "file.txt"), "one\none\n");
    await expect(editTool.execute(context(root), { path: "file.txt", oldText: "one", newText: "ONE" })).rejects.toThrow("unique");
  });

  test("rejects overlapping edits", async () => {
    const root = await mkdtemp(join(tmpdir(), "iyon-edit-overlap-"));
    await writeFile(join(root, "file.txt"), "abcdef");
    await expect(editTool.execute(context(root), { path: "file.txt", edits: [{ oldText: "abc", newText: "x" }, { oldText: "cde", newText: "y" }] })).rejects.toThrow("overlap");
  });
});
