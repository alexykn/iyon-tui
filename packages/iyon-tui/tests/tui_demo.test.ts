import { describe, expect, test } from "bun:test";
import { runTuiDemo } from "./fixtures/tui_demo.ts";

describe("T5 React TUI framework demo", () => {
	test("runs composer, content, focus, and History through public React APIs", async () => {
		const result = await runTuiDemo();
		expect(result.input).toBe("composex");
		expect(result.submitted).toBe("composex");
		expect(result.stream).toBe("streaming text");
		expect(result.screenRows.some((row) => row.includes("composer"))).toBe(
			true,
		);
		expect(
			result.screenRows.some((row) => row.includes("completed history")),
		).toBe(true);
		expect(result.editorEvents).toEqual(["input:composex", "submit:composex"]);
		expect(result.historyIdentity).not.toBe("");
		expect(["number", "string"]).toContain(typeof result.historyIdentity);
		expect(Array.isArray(result.nativeHistoryRows)).toBe(true);
		expect(result.focused).toBe(true);
	});
});
