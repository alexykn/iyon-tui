import { describe, expect, test } from "bun:test";

import { DiffHunk, DiffLine, DiffRange, DiffRenderer, View } from "../src/index.ts";
import { nodeForBridge } from "../src/values/view.ts";
import { native } from "../src/native.ts";

describe("retained TUI semantic values", () => {
  test("fluent operations return new semantic values", () => {
    const original = View.text("x");
    const styled = original.bold().padding(1).fillWidth();

    expect(original).not.toBe(styled);
    expect(original.kind).toBe("view");
    expect(styled.kind).toBe("view");
  });

  test("nested composition crosses the native boundary once", () => {
    const view = View.vertical([
      View.text("one").bold(),
      View.horizontal([View.text("two"), View.spacer(1)]),
    ]);
    const Host = native.NativeTuiHost;
    if (Host === undefined) throw new Error("native TUI host is unavailable");
    const host = new Host(20, 4, true);
    host.render(nodeForBridge(view));
    host.dispose();
  });

  test("native decoder renders semantic diff nodes", () => {
    const Host = native.NativeTuiHost;
    if (Host === undefined) throw new Error("native TUI host is unavailable");
    const host = new Host(40, 8, true);
    const diff = new DiffRenderer().render(new DiffHunk(new DiffRange(0, 1), new DiffRange(0, 1), [
      new DiffLine("context", "same"),
    ]));
    host.render(nodeForBridge(diff));
    expect(host.screenRows().some((row) => row.includes("@@ -1 +1 @@"))).toBe(true);
    expect(host.screenRows().some((row) => row.includes(" same"))).toBe(true);
    host.dispose();
  });

  test("native validation rejects malformed private nodes", () => {
    const Host = native.NativeTuiHost;
    if (Host === undefined) throw new Error("native TUI host is unavailable");
    const host = new Host(20, 4, true);
    expect(() => host.render({ id: 1, schema: 99, kind: 1 })).toThrow(/unsupported TUI View bridge schema/);
    expect(() => host.render({ id: 1, schema: 1, kind: 999 })).toThrow(/unknown numeric TUI View node kind/);
    expect(() => host.render({ id: 2, schema: 1, kind: 11, handle: 0 })).toThrow(/handle must be positive/);
    expect(() => host.render({
      id: 3,
      schema: 1,
      kind: 7,
      columns: [],
      rows: [{ track: { kind: 1 }, cells: [{ view: { id: 4, schema: 1, kind: 3, rows: 0 }, columnSpan: 0, rowSpan: 1 }] }],
      columnGap: 0,
      rowGap: 0,
    })).toThrow(/columnSpan must be positive/);
    host.dispose();
  });

  test("cache hits stop before reading the retained node payload", () => {
    const Host = native.NativeTuiHost;
    if (Host === undefined) throw new Error("native TUI host is unavailable");
    const host = new Host(20, 4, true);
    const node = nodeForBridge(View.text("cached"));
    expect(Object.isFrozen(node)).toBe(true);
    host.render(node);
    const malformed = { ...node, kind: 999 } as unknown as typeof node;
    expect(() => host.render(malformed)).not.toThrow();
    host.dispose();
  });

  test("direct decoding preserves Unicode text and arbitrary replacement", () => {
    const Host = native.NativeTuiHost;
    if (Host === undefined) throw new Error("native TUI host is unavailable");
    const host = new Host(20, 4, true);
    host.render(nodeForBridge(View.text("κ🙂")));
    expect(host.screenRows().some((row) => row.includes("κ🙂"))).toBe(true);
    host.render(nodeForBridge(View.text("replacement")));
    expect(host.screenRows().some((row) => row.includes("replacement"))).toBe(true);
    host.dispose();
  });

  test("node identity survives module re-evaluation", async () => {
    const first = nodeForBridge(View.text("first")).id;
    const reloaded = await import(`../src/values/view.ts?reload=${Date.now()}`);
    const second = reloaded.nodeForBridge(reloaded.View.text("second")).id;
    expect(second).toBeGreaterThan(first);
  });

  test("worker teardown releases its environment-local bridge cache", async () => {
    const baseline = native.tuiViewBridgeEnvironmentCount();
    for (let index = 0; index < 3; index += 1) {
      const worker = new Worker(new URL("./tui_bridge_worker.ts", import.meta.url));
      await new Promise<void>((resolve, reject) => {
        worker.onmessage = (event) => {
          if (event.data === "decoded") resolve();
          else reject(new Error(`unexpected worker message: ${String(event.data)}`));
        };
        worker.onerror = reject;
      });
      worker.terminate();
    }
    for (let attempt = 0; attempt < 20 && native.tuiViewBridgeEnvironmentCount() !== baseline; attempt += 1) {
      await new Promise((resolve) => setTimeout(resolve, 10));
    }
    expect(native.tuiViewBridgeEnvironmentCount()).toBe(baseline);
  });
});
