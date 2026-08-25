import { describe, expect, test } from "bun:test";
import { installIyonVirtualModules } from "../../../../packages/iyon-runtime/src/virtual-modules.ts";
import { nodeForBridge } from "../../../../packages/iyon-tui/src/values/view.ts";
installIyonVirtualModules();
const { bashTool } = await import("../src/execute.ts");
const context = (cwd: string, signal = new AbortController().signal) => ({ cwd, workspace: { root: cwd }, signal, update: async () => undefined } as never);

describe("bash tool", () => {
  test("compiles lifecycle and multiline result fixtures through native views", () => {
    for (const state of ["preparing", "prepared", "running"] as const) {
      expect(nodeForBridge(bashTool.renderCall({ id: "call" as never, name: "bash", arguments: { command: "echo hi" }, state }) as never)).toBeDefined();
    }
    expect(nodeForBridge(bashTool.renderResult({ content: [{ type: "text", text: "one\ntwo" }], details: {}, isError: false }) as never)).toBeDefined();
    expect(nodeForBridge(bashTool.renderResult({ content: [{ type: "text", text: "failed\nagain" }], details: { fullOutputPath: "/tmp/output" }, isError: true }) as never)).toBeDefined();
  });

  test("captures successful output and preserves exit details", async () => {
    const result = await bashTool.execute(context(process.cwd()), { command: "printf hello" });
    expect(result).toMatchObject({ content: [{ text: "hello" }], details: { exitCode: 0 }, isError: false });
  });

  test("does not fabricate success after a non-zero exit", async () => {
    const result = await bashTool.execute(context(process.cwd()), { command: "printf nope; exit 3" });
    expect(result).toMatchObject({ isError: true, details: { exitCode: 3 }, content: [{ text: expect.stringContaining("code 3") }] });
  });

  test("kills an aborted child", async () => {
    const controller = new AbortController();
    const pending = bashTool.execute(context(process.cwd(), controller.signal), { command: "sleep 5" });
    controller.abort();
    await expect(pending).rejects.toThrow("cancelled");
  });
});
