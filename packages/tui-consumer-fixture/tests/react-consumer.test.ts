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
});
