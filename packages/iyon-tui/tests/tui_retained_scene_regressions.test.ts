import { expect, test } from "bun:test";

import {
	TextFunnel,
	TextStreamSource,
	Theme,
	themeColor,
	View,
} from "../src/index.ts";
import { AppHarness } from "../src/testing/index.ts";

const RETAINED = "retained scene invalidation regressions";

function indexed(value: number) {
	return { type: "indexed" as const, value };
}

test(`${RETAINED} invalidates state dependency paths before a structural publication`, async () => {
	const tui = await AppHarness.open({ width: 20, height: 8 });
	const state = tui.viewState();
	const stable = View.vertical([View.text("child").state(state)]);
	try {
		tui.render(() => ({ body: View.vertical([stable, View.text("before")]) }));
		state.setGeometry({ padding: 1 });
		tui.render(() => ({ body: View.vertical([stable, View.text("after")]) }));

		const childRow = tui.screenRows().find((row) => row.includes("child"));
		expect(childRow).toBeDefined();
		expect(childRow?.indexOf("child")).toBe(1);
	} finally {
		tui.close();
	}
});

test(`${RETAINED} preserves a full-paint theme obligation during content refresh`, async () => {
	const tui = await AppHarness.open({ width: 24, height: 6 });
	const source = TextStreamSource.create();
	const port = tui.contentPort();
	const connector = port.connect(source, TextFunnel.plain());
	connector.activate();
	try {
		tui.setTheme(Theme.new().withColor("label", indexed(1)));
		tui.render(() => ({
			body: View.vertical([
				View.text("label").foreground(themeColor("label")),
				View.content(port),
			]),
		}));
		source.replace("before");
		tui.flush();

		tui.setTheme(Theme.new().withColor("label", indexed(2)));
		source.replace("after");
		tui.flush();

		const rows = tui.screenRows();
		const labelRow = rows.findIndex((row) => row.includes("label"));
		if (labelRow < 0) throw new Error("the themed label is not visible");
		const labelColumn = rows[labelRow]?.indexOf("label") ?? -1;
		if (labelColumn < 0)
			throw new Error("the themed label has no physical column");
		expect(tui.styleAt(labelRow, labelColumn).foreground).toBe("ansi:2");
		expect(rows.some((row) => row.includes("after"))).toBe(true);
	} finally {
		tui.close();
		source.dispose();
	}
});

test(`${RETAINED} carries captured state values through ViewSlot replacement`, async () => {
	const tui = await AppHarness.open({ width: 20, height: 4 });
	const state = tui.viewState();
	const slot = tui.createViewSlot(View.text("old"));
	try {
		state.setPresentation({ foreground: indexed(2) });
		tui.render(() => ({ body: slot.view() }));
		slot.setView(View.text("new").state(state));
		tui.flush();

		const rows = tui.screenRows();
		const row = rows.findIndex((line) => line.includes("new"));
		if (row < 0) throw new Error("the replacement view is not visible");
		const column = rows[row]?.indexOf("new") ?? -1;
		if (column < 0)
			throw new Error("the replacement view has no physical column");
		expect(tui.styleAt(row, column).foreground).toBe("ansi:2");
	} finally {
		tui.close();
	}
});
