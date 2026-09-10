import { createElement } from "react";
import { TextFunnel, TextStreamSource } from "../src/index.ts";
import { Content, createReactRoot } from "../src/react/index.ts";
import { AppHarness } from "../src/testing/index.ts";

const source = TextStreamSource.create();
const harness = await AppHarness.open({ width: 32, height: 4 });
const root = createReactRoot(harness);
try {
	const port = harness.contentPort();
	const connector = port.connect(source, TextFunnel.plain());
	await root.render(createElement(Content, { port: connector }));
	source.append("packaged TUI smoke\n");
	await root.whenContentVisible();
	const rows = harness.screenRows();
	if (!rows.some((row) => row.includes("packaged TUI smoke"))) {
		throw new Error(
			`native package smoke did not render content: ${JSON.stringify(rows)}`,
		);
	}
	console.log(
		JSON.stringify({ native: "iyon-tui-native/s6", content: "ok", rows }),
	);
	await root.unmount();
	connector.deactivate();
	connector.dispose();
	port.dispose();
	root.close();
} finally {
	harness.close();
	source.dispose();
}
