import { expect, test } from "bun:test";

import {
	TextBlockSource,
	TextFunnel,
	TextStreamSource,
	View,
} from "../src/index.ts";
import { AppHarness } from "../src/testing/index.ts";

const CACHE = "F7 semantic sequence cache ownership";

test(`${CACHE} compares every nonblank physical row across repeated Markdown replacements`, async () => {
	const tui = await AppHarness.open({ width: 32, height: 8 });
	const source = TextBlockSource.create();
	try {
		const port = tui.contentPort();
		const connector = port.connect(source, TextFunnel.markdown());
		connector.activate();
		tui.render({ body: View.content(port) });

		for (let index = 0; index < 2_500; index += 1) {
			const expected = `item-${index.toString().padStart(5, "0")}`;
			source.replace(expected);
			tui.flush();
			const nonblankRows = tui
				.screenRows()
				.map((row) => row.trimEnd())
				.filter((row) => row.trim().length > 0);
			expect(
				nonblankRows,
				`physical rows diverged at replacement ${index}`,
			).toEqual([expected]);
		}
	} finally {
		tui.close();
		source.dispose();
	}
});

test(`${CACHE} resets parser state across source content generations`, async () => {
	const cases = [
		{
			funnel: TextFunnel.markdown(),
			first: "first\n\nlast",
			second: "other\n\nlast",
			source: () => TextBlockSource.create(),
		},
		{
			funnel: TextFunnel.ansi(),
			first: "first\nlast",
			second: "other\ntail",
			source: () => TextStreamSource.create(),
		},
		{
			funnel: TextFunnel.diff(),
			first: "first\nlast",
			second: "other\ntail",
			source: () => TextStreamSource.create(),
		},
	] as const;

	for (const { funnel, first, second, source: createSource } of cases) {
		const tui = await AppHarness.open({ width: 32, height: 8 });
		const source = createSource();
		try {
			const port = tui.contentPort();
			const connector = port.connect(source, funnel);
			connector.activate();
			tui.render({ body: View.content(port) });
			const visibleRows = () =>
				tui
					.screenRows()
					.map((row) => row.trimEnd())
					.filter((row) => row.trim().length > 0);
			const expectedRows = (text: string) =>
				text.split("\n").filter((row) => row.length > 0);

			source.replace(first);
			tui.flush();
			expect(visibleRows()).toEqual(expectedRows(first));

			source.replace(second);
			tui.flush();
			expect(visibleRows()).toEqual(expectedRows(second));

			if (source instanceof TextStreamSource) {
				source.clear();
				source.append(first);
				tui.flush();
				expect(visibleRows()).toEqual(expectedRows(first));
			}
		} finally {
			tui.close();
			source.dispose();
		}
	}
});
