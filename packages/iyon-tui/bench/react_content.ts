import { createElement } from "react";
import { Column, Content, createReactRoot } from "../src/react/index.ts";
import { TextFunnel, TextStreamSource } from "../src/index.ts";
import { AppHarness } from "../src/testing/index.ts";
import { nativeHostForReact } from "../src/react/host-registry.ts";
import { native } from "../src/transport/native/addon.ts";
import type { NativeTuiHostContract } from "../src/transport/native/addon.ts";
import { UI_BATCH_HEADER_WORDS } from "../src/transport/ui/generated/ui_schema.ts";

const instrumentation = native as typeof native & {
	tuiPerfReset?: () => void;
	tuiPerfSnapshot?: () => Record<string, number>;
};
const reset = instrumentation.tuiPerfReset;
const snapshot = instrumentation.tuiPerfSnapshot;
if (reset === undefined || snapshot === undefined)
	throw new Error(
		"Build instrumentation first: ION_NATIVE_FEATURES=perf-counters bun run native:stage",
	);
const count = Number(process.env.ION_CONTENT_BENCH_COUNT ?? "1000");
if (!Number.isSafeInteger(count) || count < 1)
	throw new Error("ION_CONTENT_BENCH_COUNT must be a positive safe integer");
const harness = await AppHarness.open({ width: 80, height: 24 });
const root = createReactRoot(harness);
const host = nativeHostForReact(harness) as NativeTuiHostContract;
const source = TextStreamSource.create();
const commit = host.commitUiV1.bind(host);
let submittedUiRecords = 0;
host.commitUiV1 = (words, metadata, ownedContent, sources) => {
	for (let cursor: number = UI_BATCH_HEADER_WORDS; cursor < words.length; ) {
		const width = words[cursor + 1];
		if (width === undefined || width < 2 || cursor + width > words.length)
			throw new Error(
				"invalid UI record emitted by the production coordinator",
			);
		submittedUiRecords += 1;
		cursor += width;
	}
	return commit(words, metadata, ownedContent, sources);
};

try {
	reset();
	const start = performance.now();
	const port = harness.contentPort();
	const connector = port.connect(source, TextFunnel.plain());
	await root.render(
		createElement(
			Column,
			{
				background: { type: "named", value: "blue" },
			},
			createElement(Content, { port: connector }),
		),
	);
	for (let index = 0; index < count; index += 1)
		source.append(`line-${index}\n`);
	await root.whenContentVisible();
	const elapsedMs = performance.now() - start;
	// These are production Rust counters, not classifications of submitted UI
	// records or counts of JS inspection calls. Zero values remain meaningful.
	const nativeCounters = snapshot();
	harness.bindKey("b", "benchmark-output");
	const outputWait = harness.nextEvent();
	harness.pressKey("b");
	const output = await outputWait;
	if (output.type !== "output" || output.routeId !== "benchmark-output")
		throw new Error("native benchmark output was not delivered");
	console.log(
		JSON.stringify({
			route: "react-occurrence-content",
			appends: count,
			elapsedMs,
			submittedUiRecords,
			nativeCounters,
			deliveredOutputEvents: 1,
			visibleReceiptRevision: harness.epochs().visible_frame_revision,
		}),
	);
	await root.unmount();
	connector.deactivate();
	connector.dispose();
	port.dispose();
} finally {
	host.commitUiV1 = commit;
	root.close();
	harness.close();
	source.dispose();
}
