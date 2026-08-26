import { describe, expect, test } from "bun:test";
import { runTuiDemo } from "./fixtures/tui_demo.ts";

describe("T5 TS-only TUI framework demo", () => {
  test("runs composer, history, stream, focus, and Scene through public APIs", async () => {
    const result = await runTuiDemo();
    expect(result.input).toBe("compose");
    expect(result.stream).toBe("streaming text");
    expect(result.screenRows.length).toBeGreaterThan(0);
    expect(result.nativeHistoryRows.length).toBeGreaterThan(0);
    expect(result.focused).toBe(true);
  });
});
