import { describe, expect, test } from "bun:test";
import { openReactConsumerSession } from "../src/react-consumer.ts";

describe("public React consumer fixture", () => {
	test("uses only public entrypoints and receives native acceptance", async () => {
		const session = await openReactConsumerSession();
		try {
			const result = await session.render({
				title: "react fixture",
				status: "ready",
				items: ["one"],
				showHint: true,
			});
			expect(result.accepted).toBe(true);
			expect(result.revision).toBeGreaterThan(0);
			expect(session.root.faulted).toBe(false);
		} finally {
			await session.close();
		}
	});

	test("observes editor callbacks through public focus and preserves History identity", async () => {
		const session = await openReactConsumerSession();
		try {
			await session.render({
				title: "react fixture",
				status: "ready",
				items: ["one"],
				showHint: true,
			});
			const identity = session.historyIdentity();
			session.focusEditor();
			const submitted = session.waitForSubmit();
			session.tui.pressKey("x");
			session.tui.pressKey("Enter");
			expect(await submitted).toBe("composex");
			expect(session.lastInput()).toBe("composex");
			expect(session.editorEvents()).toEqual([
				"input:composex",
				"submit:composex",
			]);

			await session.render({
				title: "react fixture",
				status: "updated",
				items: ["one"],
				showHint: false,
			});
			expect(session.historyIdentity()).toBe(identity);
		} finally {
			await session.close();
		}
	});

	test("uses keyed Scroll/Animation and Source-Port-Connector updates", async () => {
		const session = await openReactConsumerSession();
		try {
			await session.render({
				title: "react fixture",
				status: "ready",
				items: ["one", "two"],
				showHint: true,
			});
			expect(
				session.tui.screenRows().some((row) => row.includes("- one")),
			).toBe(true);
			expect(
				session.tui.screenRows().some((row) => row.includes("animation-a")),
			).toBe(true);

			const before = session.source.stats();
			session.source.append("streaming text");
			await session.root.whenContentVisible();
			expect(session.source.stats().revision).toBeGreaterThan(before.revision);
			expect(
				session.tui.screenRows().some((row) => row.includes("streaming text")),
			).toBe(true);

			session.tui.advance(20);
			// Native deadlines do not advance the React UI revision.
			const animationDeadline = performance.now() + 1_000;
			while (
				!session.tui.screenRows().some((row) => row.includes("animation-b")) &&
				performance.now() < animationDeadline
			) {
				await Bun.sleep(1);
			}
			expect(
				session.tui.screenRows().some((row) => row.includes("animation-b")),
			).toBe(true);

			await session.render({
				title: "react fixture",
				status: "updated",
				items: ["two", "three"],
				showHint: false,
			});
			const screen = session.tui.screenRows().join("\n");
			expect(screen).toContain("- two");
			expect(screen).toContain("- three");
			expect(screen).not.toContain("- one");
		} finally {
			await session.close();
		}
		expect(session.source.disposed).toBe(true);
	});
});
