import { describe, expect, test } from "bun:test";

import { DiffHunk, DiffLine, DiffRange, DiffRenderer, View } from "../src/index.ts";
import { native } from "../src/transport/native/addon.ts";
import {
  viewReleaseMany,
  viewTextCreateBuffer,
} from "../src/transport/abi/structural/generated/view_calls.ts";
import {
  nativeViewAbiSession,
  tryRetainedMaterializeRef,
} from "../src/transport/structural/native-view-abi.ts";
import { nodeIdPair, viewNodeId } from "../src/api/view/view.ts";
import { AppHarness } from "../src/testing/index.ts";
import { renderRetained } from "./fixtures/native-host.ts";

describe("retained TUI semantic values", () => {
  test("fluent operations return new semantic values", () => {
    const original = View.text("x");
    const styled = original.bold().padding(1).fillWidth();

    expect(original).not.toBe(styled);
    expect(original.kind).toBe("view");
    expect(styled.kind).toBe("view");
  });

  test("nested composition crosses the native boundary once", async () => {
    const view = View.vertical([
      View.text("one").bold(),
      View.horizontal([View.text("two"), View.spacer(1)]),
    ]);
    const tui = await AppHarness.open({ width: 20, height: 4 });
    try {
      tui.render({ body: view });
      const screen = tui.screenRows().join("\n");
      expect(screen).toContain("one");
      expect(screen).toContain("two");
    } finally {
      tui.close();
    }
  });

  test("retained path renders semantic diff nodes", async () => {
    const diff = new DiffRenderer().render(new DiffHunk(new DiffRange(0, 1), new DiffRange(0, 1), [
      new DiffLine("context", "same"),
    ]));
    const tui = await AppHarness.open({ width: 40, height: 8 });
    try {
      tui.render({ body: diff });
      expect(tui.screenRows().some((row) => row.includes("@@ -1 +1 @@"))).toBe(true);
      expect(tui.screenRows().some((row) => row.includes(" same"))).toBe(true);
    } finally {
      tui.close();
    }
  });

  test("retained validation rejects malformed buffer payloads", () => {
    const session = nativeViewAbiSession();
    if (session === undefined) return;
    // Span count disagrees with the framed pairs.
    const view = View.text("malformed-span-count");
    const [low, high] = nodeIdPair(view);
    const words = new Uint32Array([5, 0, 3]);
    const bytes = new Uint8Array([97, 98, 99]);
    expect(() =>
      viewTextCreateBuffer(session.symbols, session.runtime, low, high, words, 3, bytes, 3, 3, 1),
    ).toThrow();
    // Empty span lists have no valid rendering.
    const empty = View.text("malformed-empty");
    const [emptyLow, emptyHigh] = nodeIdPair(empty);
    expect(() =>
      viewTextCreateBuffer(
        session.symbols,
        session.runtime,
        emptyLow,
        emptyHigh,
        new Uint32Array([0]),
        1,
        new Uint8Array(0),
        0,
        3,
        1,
      ),
    ).toThrow();
  });

  test("cache hits stop before reading the retained node payload", () => {
    const session = nativeViewAbiSession();
    if (session === undefined) return;
    const view = View.text("cached");
    const reference = tryRetainedMaterializeRef(view);
    if (reference === undefined) throw new Error("retained materialization refused the view");
    try {
      const [low, high] = nodeIdPair(view);
      // The same NodeId with a corrupt payload still resolves: the live
      // semantic-cache entry wins before any payload byte is read.
      const corrupt = new Uint32Array([0]);
      const cached = viewTextCreateBuffer(
        session.symbols,
        session.runtime,
        low,
        high,
        corrupt,
        corrupt.length,
        new Uint8Array(0),
        0,
        1,
        1,
      );
      expect(cached).toBe(reference);
    } finally {
      viewReleaseMany(session.symbols, session.runtime, new Uint32Array([reference]), 1);
    }
  });

  test("generated host-ref rendering preserves Unicode text and arbitrary replacement", () => {
    const Host = native.NativeTuiHost;
    if (Host === undefined) throw new Error("native TUI host is unavailable");
    const host = new Host(20, 4, true);
    renderRetained(host, View.text("κ🙂"));
    expect(host.screenRows().some((row) => row.includes("κ🙂"))).toBe(true);
    renderRetained(host, View.text("replacement"));
    expect(host.screenRows().some((row) => row.includes("replacement"))).toBe(true);
    host.dispose();
  });

  test("node identity survives module re-evaluation", async () => {
    const first = viewNodeId(View.text("first"));
    const reloaded = await import(`../src/api/view/view.ts?reload=${Date.now()}`);
    const second = viewNodeId(reloaded.View.text("second"));
    expect(second).toBeGreaterThan(first);
  });

  test("worker teardown releases its environment-local native session", async () => {
    const baseline = native.tuiViewEnvironmentCount();
    for (let index = 0; index < 3; index += 1) {
      const worker = new Worker(new URL("./tui_worker_lifecycle.ts", import.meta.url));
      await new Promise<void>((resolve, reject) => {
        worker.onmessage = (event) => {
          if (event.data === "ready") resolve();
          else reject(new Error(`unexpected worker message: ${String(event.data)}`));
        };
        worker.onerror = reject;
      });
      worker.terminate();
    }
    for (let attempt = 0; attempt < 20 && native.tuiViewEnvironmentCount() !== baseline; attempt += 1) {
      await new Promise((resolve) => setTimeout(resolve, 10));
    }
    expect(native.tuiViewEnvironmentCount()).toBe(baseline);
  });
});
