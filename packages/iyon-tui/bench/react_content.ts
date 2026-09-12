import { createElement, type ReactNode } from "react";
import {
	StyleSpec,
	TextFunnel,
	TextStreamSource,
	Theme,
} from "../src/index.ts";
import { nativeHostForReact } from "../src/react/host-registry.ts";
import {
	Animation,
	Box,
	Column,
	Content,
	createReactRoot,
	Editor,
	type OccurrenceRef,
	Row,
	Scroll,
	Text,
} from "../src/react/index.ts";
import { AppHarness } from "../src/testing/index.ts";
import {
	type NativeTuiHostContract,
	native,
	nativeArtifact,
} from "../src/transport/native/addon.ts";
import {
	UI_BATCH_HEADER_WORDS,
	UI_OPCODE_DESCRIPTORS,
} from "../src/transport/ui/generated/ui_schema.ts";

/**
 * This benchmark intentionally measures the production React/occurrence/
 * content path. The default addon provides current-source JavaScript traffic
 * and barrier timings; the opt-in perf-counters addon adds native-owned stage
 * timings and counters. Archived comparisons must run the archived source
 * benchmark with its matching addon, never this current-source entrypoint.
 */
if (process.env.ION_TUI_NATIVE_ARTIFACT !== undefined)
	throw new Error(
		"react_content.ts refuses an external native artifact override; run an archived source benchmark with its matching source/addon pair instead",
	);
const instrumentedNative = native as typeof native & {
	tuiPerfReset?: () => void;
	tuiPerfSnapshot?: () => Record<string, number>;
};
const perfReset = instrumentedNative.tuiPerfReset;
const perfSnapshot = instrumentedNative.tuiPerfSnapshot;
if ((perfReset === undefined) !== (perfSnapshot === undefined))
	throw new Error("native performance instrumentation exports are incomplete");
if (
	process.env.ION_PERF_REQUIRE_COUNTERS === "1" &&
	(perfReset === undefined || perfSnapshot === undefined)
)
	throw new Error(
		"Build instrumentation first: ION_NATIVE_FEATURES=perf-counters bun run native:stage",
	);

const warmupCount = positiveEnv("ION_PERF_WARMUPS", 1, 0, 5);
const sampleCount = positiveEnv("ION_PERF_SAMPLES", 7, 3, 20);
const appendCount = positiveEnv("ION_CONTENT_BENCH_COUNT", 16, 1, 256);
const outputPath = process.env.ION_PERF_OUTPUT;
const opcodeNames = new Map<number, string>(
	UI_OPCODE_DESCRIPTORS.map(({ code, name }) => [code, name]),
);

interface TrafficSnapshot {
	calls: number;
	records: number;
	transportBytes: number;
	semanticBytes: number;
	wordBytes: number;
	metadataBytes: number;
	contentBytes: number;
	nativeAcceptanceMs: number;
	opcodes: Readonly<Record<string, number>>;
}

interface TrafficCapture {
	reset(): void;
	snapshot(): TrafficSnapshot;
	restore(): void;
}

interface PendingWork {
	readonly accepted: Promise<void>;
	readonly visible: Promise<void>;
}

interface BenchmarkSample {
	readonly sample: number;
	readonly acceptedMs: number;
	readonly postAcceptanceBarrierMs: number;
	readonly endToEndMs: number;
	readonly frontendCallbackNormalizationMs: number;
	readonly nativeAcceptanceMs: number;
	readonly barrier: "visible" | "content-visible";
	readonly traffic: TrafficSnapshot;
	readonly nativeCounters: Readonly<Record<string, number>>;
	readonly memory?: Readonly<Record<string, number | string>>;
}

interface Session {
	readonly context: Context;
	readonly mountMs: number;
	readonly barrier: "visible" | "content-visible";
	prepare?(): Promise<void>;
	mutate(sample: number): PendingWork;
	memory?(): Readonly<Record<string, number | string>>;
	close(): Promise<void>;
}

interface SessionDefinition {
	readonly children: ReactNode;
	readonly content?: boolean;
	readonly barrier: "visible" | "content-visible";
	prepare?(): Promise<void>;
	mutate(sample: number): PendingWork;
	memory?(): Readonly<Record<string, number | string>>;
	cleanup?(): void;
}

interface Context {
	readonly harness: AppHarness;
	readonly root: ReturnType<typeof createReactRoot>;
	readonly host: NativeTuiHostContract;
	readonly capture: TrafficCapture;
}

function positiveEnv(
	name: string,
	defaultValue: number,
	minimum: number,
	maximum: number,
): number {
	const value = Number(process.env[name] ?? defaultValue);
	if (!Number.isSafeInteger(value) || value < minimum || value > maximum)
		throw new Error(
			`${name} must be an integer from ${minimum} through ${maximum}`,
		);
	return value;
}

function assertCondition(
	condition: unknown,
	message: string,
): asserts condition {
	if (!condition) throw new Error(message);
}

function decodeRecordOpcodes(words: Uint32Array): number[] {
	const opcodes: number[] = [];
	let cursor = UI_BATCH_HEADER_WORDS;
	for (let section = 0; section < 5; section += 1) {
		const sectionLength = words[7 + section];
		assertCondition(
			sectionLength !== undefined,
			"UI batch omitted a section length",
		);
		const end = cursor + sectionLength;
		assertCondition(
			end <= words.length,
			"UI batch section exceeds its word buffer",
		);
		while (cursor < end) {
			const opcode = words[cursor];
			const width = words[cursor + 1];
			assertCondition(
				opcode !== undefined && width !== undefined,
				"UI record is truncated",
			);
			assertCondition(
				width >= 2 && cursor + width <= end,
				"UI record width is invalid",
			);
			opcodes.push(opcode);
			cursor += width;
		}
		assertCondition(
			cursor === end,
			"UI section did not terminate on a record boundary",
		);
	}
	return opcodes;
}

function instrumentHost(host: NativeTuiHostContract): TrafficCapture {
	const originalCommit = host.commitUiV1.bind(host);
	let calls = 0;
	let records = 0;
	let transportBytes = 0;
	let wordBytes = 0;
	let metadataBytes = 0;
	let contentBytes = 0;
	let nativeAcceptanceMs = 0;
	const opcodes = new Map<number, number>();
	const reset = () => {
		calls = 0;
		records = 0;
		transportBytes = 0;
		wordBytes = 0;
		metadataBytes = 0;
		contentBytes = 0;
		nativeAcceptanceMs = 0;
		opcodes.clear();
	};
	host.commitUiV1 = (words, metadata, content, sources) => {
		const decoded = decodeRecordOpcodes(words);
		const started = performance.now();
		try {
			return originalCommit(words, metadata, content, sources);
		} finally {
			calls += 1;
			records += decoded.length;
			transportBytes +=
				words.byteLength + metadata.byteLength + content.byteLength;
			wordBytes += words.byteLength;
			metadataBytes += metadata.byteLength;
			contentBytes += content.byteLength;
			nativeAcceptanceMs += performance.now() - started;
			for (const opcode of decoded)
				opcodes.set(opcode, (opcodes.get(opcode) ?? 0) + 1);
		}
	};
	return {
		reset,
		snapshot: () => ({
			calls,
			records,
			transportBytes,
			semanticBytes: metadataBytes + contentBytes,
			wordBytes,
			metadataBytes,
			contentBytes,
			nativeAcceptanceMs,
			opcodes: Object.fromEntries(
				[...opcodes.entries()].map(([opcode, count]) => [
					opcodeNames.get(opcode) ?? `opcode-${opcode}`,
					count,
				]),
			),
		}),
		restore: () => {
			host.commitUiV1 = originalCommit;
		},
	};
}

async function openContext(width: number, height: number): Promise<Context> {
	const harness = await AppHarness.open({ width, height });
	try {
		const root = createReactRoot(harness);
		const host = nativeHostForReact(harness) as
			| NativeTuiHostContract
			| undefined;
		assertCondition(
			host !== undefined,
			"native host association is unavailable",
		);
		return { harness, root, host, capture: instrumentHost(host) };
	} catch (error) {
		harness.close();
		throw error;
	}
}

async function disposeContext(
	context: Context,
	postUnmount?: () => void,
): Promise<void> {
	let failure: unknown;
	try {
		await context.root.unmount();
	} catch (error) {
		failure = error;
	}
	try {
		postUnmount?.();
	} catch (error) {
		failure = failure ?? error;
	}
	try {
		context.root.close();
	} catch (error) {
		failure = failure ?? error;
	}
	context.capture.restore();
	context.harness.close();
	if (failure !== undefined) throw failure;
}

async function mount(
	context: Context,
	children: ReactNode,
	content = false,
): Promise<number> {
	const started = performance.now();
	const commit = await context.root.render(children);
	if (content) await context.root.whenContentVisible(commit.revision);
	else await context.root.whenVisible(commit.revision);
	return performance.now() - started;
}

async function newSession(
	width: number,
	height: number,
	define: (context: Context) => SessionDefinition | Promise<SessionDefinition>,
): Promise<Session> {
	const context = await openContext(width, height);
	let definition: SessionDefinition | undefined;
	try {
		definition = await define(context);
		const mountMs = await mount(
			context,
			definition.children,
			definition.content ?? false,
		);
		return {
			context,
			mountMs,
			barrier: definition.barrier,
			prepare: definition.prepare,
			mutate: definition.mutate,
			memory: definition.memory,
			close: () => disposeContext(context, definition?.cleanup),
		};
	} catch (error) {
		await disposeContext(context, definition?.cleanup).catch(() => undefined);
		throw error;
	}
}

function reactWork(
	context: Context,
	children: ReactNode,
	content = false,
): PendingWork {
	const accepted = context.root.render(children);
	const visible = accepted.then(({ revision }) =>
		content
			? context.root.whenContentVisible(revision)
			: context.root.whenVisible(revision),
	);
	return { accepted: accepted.then(() => undefined), visible };
}

function barrierWork(context: Context, content = false): PendingWork {
	return {
		accepted: Promise.resolve(),
		visible: content
			? context.root.whenContentVisible()
			: context.root.whenVisible(),
	};
}

function selectedNativeCounters(): Readonly<Record<string, number>> {
	const counters = perfSnapshot?.() ?? {};
	// Only counters with a current production owner are reported. In
	// particular, removed renderer counters are not emitted as misleading zeroes.
	const owned = [
		"source_snapshots_acquired",
		"semantic_preparations",
		"semantic_projection_rebuilds",
		"content_wake_groups",
		"content_due_connectors",
		"content_candidate_records_prepared",
		"content_demand_nodes_visited",
		"content_owner_nodes_visited",
		"content_metric_changes",
		"content_paint_propagations",
		"surface_cells_composited",
		"ui_control_keys_visited",
		"frame_prepare_nanos",
		"runtime_advance_nanos",
		"frame_present_nanos",
		"frame_commit_nanos",
		"direct_capture_nanos",
		"direct_refinement_nanos",
		"direct_driver_layout_nanos",
		"taffy_layout_nanos",
		"taffy_layout_passes",
		"direct_paint_nanos",
	] as const;
	return Object.fromEntries(
		owned.flatMap((name) =>
			counters[name] === undefined ? [] : [[name, counters[name]]],
		),
	);
}

function quantile(values: readonly number[], probability: number): number {
	const sorted = [...values].sort((left, right) => left - right);
	if (sorted.length === 0) return 0;
	const position = (sorted.length - 1) * probability;
	const lower = Math.floor(position);
	const upper = Math.ceil(position);
	const lowerValue = sorted[lower] ?? 0;
	const upperValue = sorted[upper] ?? lowerValue;
	return lowerValue + (upperValue - lowerValue) * (position - lower);
}

function sampleQuantiles(samples: readonly BenchmarkSample[]) {
	const values = (key: keyof BenchmarkSample): Record<string, number> => {
		const numbers = samples.map((sample) => sample[key] as number);
		return {
			p50: quantile(numbers, 0.5),
			p95: quantile(numbers, 0.95),
			p99: quantile(numbers, 0.99),
		};
	};
	return {
		acceptedMs: values("acceptedMs"),
		postAcceptanceBarrierMs: values("postAcceptanceBarrierMs"),
		endToEndMs: values("endToEndMs"),
		frontendCallbackNormalizationMs: values("frontendCallbackNormalizationMs"),
		nativeAcceptanceMs: values("nativeAcceptanceMs"),
		p99Stability:
			samples.length >= 20 ? "reviewable" : "exploratory-max-adjacent",
	};
}

async function runSession(name: string, session: Session) {
	try {
		for (let warmup = 0; warmup < warmupCount; warmup += 1) {
			await session.prepare?.();
			perfReset?.();
			const pending = session.mutate(warmup);
			await pending.accepted;
			await pending.visible;
		}
		const samples: BenchmarkSample[] = [];
		for (let sample = 0; sample < sampleCount; sample += 1) {
			await session.prepare?.();
			session.context.capture.reset();
			perfReset?.();
			const started = performance.now();
			const pending = session.mutate(sample + warmupCount);
			await pending.accepted;
			const acceptedMs = performance.now() - started;
			const barrierStarted = performance.now();
			await pending.visible;
			const endToEndMs = performance.now() - started;
			const postAcceptanceBarrierMs = performance.now() - barrierStarted;
			const traffic = session.context.capture.snapshot();
			samples.push({
				sample,
				acceptedMs,
				postAcceptanceBarrierMs,
				endToEndMs,
				frontendCallbackNormalizationMs: Math.max(
					0,
					acceptedMs - traffic.nativeAcceptanceMs,
				),
				nativeAcceptanceMs: traffic.nativeAcceptanceMs,
				barrier: session.barrier,
				traffic,
				nativeCounters: selectedNativeCounters(),
				...(session.memory === undefined ? {} : { memory: session.memory() }),
			});
		}
		return {
			name,
			mountMs: session.mountMs,
			sampleCount: samples.length,
			samples,
			quantiles: sampleQuantiles(samples),
		};
	} finally {
		await session.close();
	}
}

function stableTree(background: "red" | "blue", rows = 48): ReactNode {
	return createElement(
		Column,
		{ width: "fill" },
		...Array.from({ length: rows }, (_, index) =>
			createElement(
				Box,
				{
					key: `stable-${index}`,
					background: {
						type: "named",
						value: index === rows - 1 ? background : "black",
					},
				},
				createElement(Text, {}, `stable row ${index}`),
			),
		),
	);
}

async function openStableSession(): Promise<Session> {
	return newSession(120, 48, (context) => ({
		children: stableTree("red"),
		barrier: "visible",
		mutate: (sample) =>
			reactWork(context, stableTree(sample % 2 === 0 ? "blue" : "red")),
	}));
}

function keyedTree(items: readonly string[]): ReactNode {
	return createElement(
		Row,
		{ width: "fill" },
		...items.map((item) => createElement(Text, { key: item }, item)),
	);
}

async function openKeyedMoveSession(): Promise<Session> {
	const items = Array.from({ length: 64 }, (_, index) => `key-${index}`);
	return newSession(120, 12, (context) => ({
		children: keyedTree(items),
		barrier: "visible",
		mutate: (sample) =>
			reactWork(
				context,
				keyedTree(sample % 2 === 0 ? [...items].reverse() : items),
			),
	}));
}

function deepTree(inserted: boolean): ReactNode {
	const leaf = inserted
		? [
				createElement(Text, { key: "anchor" }, "anchor"),
				createElement(Text, { key: "inserted" }, "inserted"),
			]
		: [createElement(Text, { key: "anchor" }, "anchor")];
	let result: ReactNode = createElement(Box, { key: "deep-leaf" }, ...leaf);
	for (let depth = 0; depth < 12; depth += 1)
		result = createElement(Box, { key: `deep-${depth}` }, result);
	return result;
}

function appendMarkdownBatch(
	source: TextStreamSource,
	harness: AppHarness,
	sample: number,
	smooth: boolean,
	steady: boolean,
): void {
	const lines = Array.from(
		{ length: appendCount },
		(_, index) =>
			`- **${steady ? "steady" : "burst"} ${sample}** item ${index}\n`,
	);
	if (steady) {
		for (const line of lines) {
			source.append(line);
			if (smooth) harness.advance(10);
		}
		return;
	}
	source.append(lines.join(""));
	if (smooth) harness.advance(10);
}

async function openDeepSession(): Promise<Session> {
	return newSession(80, 24, (context) => ({
		children: deepTree(false),
		barrier: "visible",
		mutate: (sample) => reactWork(context, deepTree(sample % 2 === 0)),
	}));
}

async function openSourceSession(width: number): Promise<Session> {
	const seed = "source line\n".repeat(8);
	return newSession(width, 24, (context) => {
		const source = TextStreamSource.create();
		const port = context.harness.contentPort();
		const connector = port.connect(source, TextFunnel.plain({ wrap: "word" }));
		source.append(seed);
		return {
			children: createElement(Content, { port: connector }),
			content: true,
			barrier: "content-visible",
			prepare: async () => {
				source.replace(seed);
				await context.root.whenContentVisible();
			},
			mutate: (sample) => {
				source.append(
					Array.from(
						{ length: appendCount },
						(_, index) => `width-${width}-sample-${sample}-${index}\n`,
					).join(""),
				);
				return barrierWork(context, true);
			},
			memory: () => {
				const stats = source.stats();
				return {
					width,
					sourceRetainedBytes: stats.retainedBytes.toString(),
					sourceCopiedBytes: stats.copiedBytes.toString(),
					sourceAcceptedBytes: stats.acceptedBytes.toString(),
					processRssBytes: process.memoryUsage().rss,
					jsHeapUsedBytes: process.memoryUsage().heapUsed,
				};
			},
			cleanup: () => {
				connector.deactivate();
				connector.dispose();
				port.dispose();
				source.dispose();
			},
		};
	});
}

async function openMarkdownSession(
	smooth: boolean,
	steady = false,
): Promise<Session> {
	const seed = "# Markdown\n\nInitial paragraph.\n";
	return newSession(80, 24, (context) => {
		const source = TextStreamSource.create();
		const port = context.harness.contentPort();
		const funnel = TextFunnel.markdown(
			smooth ? { smooth: { tickIntervalMs: 10 } } : { smooth: false },
		);
		const connector = port.connect(source, funnel);
		source.append(seed);
		return {
			children: createElement(Content, { port: connector }),
			content: true,
			barrier: "content-visible",
			prepare: async () => {
				source.replace(seed);
				await context.root.whenContentVisible();
			},
			mutate: (sample) => {
				appendMarkdownBatch(source, context.harness, sample, smooth, steady);
				return barrierWork(context, true);
			},
			memory: () => {
				const stats = source.stats();
				return {
					sourceRetainedBytes: stats.retainedBytes.toString(),
					sourceCopiedBytes: stats.copiedBytes.toString(),
					sourceAcceptedBytes: stats.acceptedBytes.toString(),
					processRssBytes: process.memoryUsage().rss,
					jsHeapUsedBytes: process.memoryUsage().heapUsed,
				};
			},
			cleanup: () => {
				connector.deactivate();
				connector.dispose();
				port.dispose();
				source.dispose();
			},
		};
	});
}

async function openEditorSession(): Promise<Session> {
	return newSession(40, 12, (context) => ({
		children: createElement(Editor, { defaultValue: "editor" }),
		barrier: "visible",
		mutate: () => {
			context.harness.pressKey("x");
			return barrierWork(context);
		},
	}));
}

async function openAnimationSession(): Promise<Session> {
	return newSession(40, 12, (context) => ({
		children: createElement(
			Animation,
			{ intervalMs: 10 },
			createElement(Box, {}, createElement(Text, {}, "animation-a")),
			createElement(Box, {}, createElement(Text, {}, "animation-b")),
		),
		barrier: "visible",
		mutate: () => {
			context.harness.advance(10);
			return barrierWork(context);
		},
	}));
}

async function openResizeThemeSession(): Promise<Session> {
	return newSession(80, 24, (context) => {
		let scrollRef: OccurrenceRef | undefined;
		return {
			children: createElement(
				Scroll,
				{
					ref: (value) => {
						scrollRef = value ?? undefined;
					},
				},
				...Array.from({ length: 24 }, (_, index) =>
					createElement(Text, { key: index }, `scroll row ${index}`),
				),
			),
			barrier: "visible",
			mutate: (sample) => {
				context.harness.resize(sample % 2 === 0 ? 100 : 80, 24);
				scrollRef?.focus();
				context.harness.pressKey("PageDown");
				context.harness.setTheme(
					Theme.new().withStyle(
						"benchmark",
						new StyleSpec().foreground({ type: "named", value: "cyan" }),
					),
				);
				return barrierWork(context);
			},
		};
	});
}

async function openMountUnmountSession(): Promise<Session> {
	const children = stableTree("red", 12);
	return newSession(80, 24, (context) => ({
		children,
		barrier: "visible",
		mutate: () => {
			const unmounted = context.root.unmount();
			const accepted = unmounted.then(() => context.root.render(children));
			const visible = accepted.then(({ revision }) =>
				context.root.whenVisible(revision),
			);
			return { accepted: accepted.then(() => undefined), visible };
		},
	}));
}

async function openSharedMemorySession(): Promise<Session> {
	return newSession(80, 24, (context) => {
		const source = TextStreamSource.create();
		const firstPort = context.harness.contentPort();
		const secondPort = context.harness.contentPort();
		const first = firstPort.connect(source, TextFunnel.plain());
		const second = secondPort.connect(source, TextFunnel.plain());
		const one = createElement(Content, { key: "one", port: first });
		const two = createElement(Content, { key: "two", port: second });
		let occurrences = 1;
		source.append("shared source\n");
		return {
			children: one,
			content: true,
			barrier: "content-visible",
			mutate: (sample) => {
				occurrences = sample % 2 === 0 ? 2 : 1;
				return reactWork(
					context,
					occurrences === 2 ? createElement(Column, {}, one, two) : one,
					true,
				);
			},
			memory: () => {
				const stats = source.stats();
				return {
					occurrences,
					sourceRetainedBytes: stats.retainedBytes.toString(),
					sourceCopiedBytes: stats.copiedBytes.toString(),
					sourceAcceptedBytes: stats.acceptedBytes.toString(),
					processRssBytes: process.memoryUsage().rss,
					jsHeapUsedBytes: process.memoryUsage().heapUsed,
				};
			},
			cleanup: () => {
				first.deactivate();
				second.deactivate();
				first.dispose();
				second.dispose();
				firstPort.dispose();
				secondPort.dispose();
				source.dispose();
			},
		};
	});
}

function witnessTraffic(
	name: string,
	traffic: TrafficSnapshot,
): Readonly<Record<string, unknown>> {
	console.error(`Measured traffic: ${name}`);
	return { name, ...traffic };
}

function assertZeroTraffic(name: string, traffic: TrafficSnapshot): void {
	assertCondition(
		traffic.calls === 0,
		`${name}: expected zero native UI calls`,
	);
	assertCondition(traffic.records === 0, `${name}: expected zero UI records`);
	assertCondition(
		traffic.semanticBytes === 0,
		`${name}: expected zero semantic bytes`,
	);
}

async function runTrafficWitnesses(): Promise<
	readonly Readonly<Record<string, unknown>>[]
> {
	const witnesses: Readonly<Record<string, unknown>>[] = [];
	const context = await openContext(40, 10);
	const callback = () => {};
	const renderTree = (
		background: "red" | "blue",
		handler: () => void,
		items: readonly (string | readonly [string, string])[],
	): ReactNode =>
		createElement(
			Box,
			{ background: { type: "named", value: background }, onPress: handler },
			...items.map((item) =>
				createElement(
					Text,
					{ key: typeof item === "string" ? item : item[0] },
					typeof item === "string" ? item : item[1],
				),
			),
		);
	try {
		await mount(context, renderTree("red", callback, ["a", "b", "c", "d"]));
		context.capture.reset();
		const noOpCommit = await context.root.render(
			renderTree("red", callback, ["a", "b", "c", "d"]),
		);
		await context.root.whenVisible(noOpCommit.revision);
		const noOp = context.capture.snapshot();
		assertZeroTraffic("same-normalized-output", noOp);
		witnesses.push(witnessTraffic("same-normalized-output", noOp));

		context.capture.reset();
		const callbackCommit = await context.root.render(
			renderTree("red", () => {}, ["a", "b", "c", "d"]),
		);
		await context.root.whenVisible(callbackCommit.revision);
		const callbackOnly = context.capture.snapshot();
		assertZeroTraffic("callback-identity-only", callbackOnly);
		witnesses.push(witnessTraffic("callback-identity-only", callbackOnly));

		context.capture.reset();
		const styleCommit = await context.root.render(
			renderTree("blue", callback, ["a", "b", "c", "d"]),
		);
		await context.root.whenVisible(styleCommit.revision);
		const localStyle = context.capture.snapshot();
		assertCondition(
			localStyle.calls === 1 &&
				localStyle.records === 1 &&
				localStyle.opcodes.SetDeclared === 1 &&
				localStyle.semanticBytes === 0,
			"local-style: expected one state property delta and no payload",
		);
		witnesses.push(witnessTraffic("local-style", localStyle));

		context.capture.reset();
		const replacementCommit = await context.root.render(
			renderTree("blue", callback, [["a", "a"], ["b", "changed"], "c", "d"]),
		);
		await context.root.whenVisible(replacementCommit.revision);
		const staticReplacement = context.capture.snapshot();
		assertCondition(
			staticReplacement.records === 1 &&
				staticReplacement.opcodes.ReplaceLiteral === 1 &&
				staticReplacement.contentBytes > 0 &&
				staticReplacement.metadataBytes === 0,
			"static-text-replacement: expected content payload only",
		);
		witnesses.push(
			witnessTraffic("static-text-replacement", staticReplacement),
		);

		context.capture.reset();
		const moveCommit = await context.root.render(
			renderTree("blue", callback, ["d", "c", ["b", "changed"], "a"]),
		);
		await context.root.whenVisible(moveCommit.revision);
		const keyedMove = context.capture.snapshot();
		assertCondition(
			keyedMove.records > 0 &&
				Object.keys(keyedMove.opcodes).every(
					(opcode) => opcode === "InsertBefore",
				) &&
				keyedMove.semanticBytes === 0,
			"keyed-move: expected placements only",
		);
		witnesses.push(witnessTraffic("keyed-move", keyedMove));
	} finally {
		await disposeContext(context);
	}

	const sourceContext = await openContext(40, 10);
	const source = TextStreamSource.create();
	const sourcePort = sourceContext.harness.contentPort();
	const sourceConnector = sourcePort.connect(source, TextFunnel.plain());
	try {
		source.append("source\n");
		await mount(
			sourceContext,
			createElement(Content, { port: sourceConnector }),
			true,
		);
		sourceContext.capture.reset();
		const sourceBefore = source.stats();
		source.append("append\n");
		await sourceContext.root.whenContentVisible();
		const sourceAfter = source.stats();
		const sourceAppend = sourceContext.capture.snapshot();
		assertZeroTraffic("source-append", sourceAppend);
		const acceptedBytes = Number(
			sourceAfter.acceptedBytes - sourceBefore.acceptedBytes,
		);
		const copiedBytes = Number(
			sourceAfter.copiedBytes - sourceBefore.copiedBytes,
		);
		assertCondition(
			acceptedBytes === 7 && copiedBytes === 7,
			"source-append: expected the seven-byte UTF-8 mutation to be accepted and copied",
		);
		witnesses.push({
			...witnessTraffic("source-append", sourceAppend),
			sourceMutationBytes: { acceptedBytes, copiedBytes },
		});
	} finally {
		await disposeContext(sourceContext, () => {
			sourceConnector.deactivate();
			sourceConnector.dispose();
			sourcePort.dispose();
			source.dispose();
		});
	}

	const animationContext = await openContext(40, 10);
	try {
		await mount(
			animationContext,
			createElement(
				Animation,
				{ intervalMs: 10 },
				createElement(Box, {}, createElement(Text, {}, "animation-a")),
				createElement(Box, {}, createElement(Text, {}, "animation-b")),
			),
		);
		animationContext.capture.reset();
		animationContext.harness.advance(10);
		await animationContext.root.whenVisible();
		const animation = animationContext.capture.snapshot();
		assertZeroTraffic("native-animation", animation);
		witnesses.push(witnessTraffic("native-animation", animation));
	} finally {
		await disposeContext(animationContext);
	}

	const recolorContext = await openContext(40, 10);
	try {
		await mount(recolorContext, createElement(Text, {}, "recolor"));
		recolorContext.capture.reset();
		recolorContext.harness.setTheme(
			Theme.new().withStyle(
				"benchmark",
				new StyleSpec().foreground({ type: "named", value: "green" }),
			),
		);
		await recolorContext.root.whenVisible();
		const recolor = recolorContext.capture.snapshot();
		assertZeroTraffic("native-environment-recolor", recolor);
		witnesses.push(witnessTraffic("native-environment-recolor", recolor));
	} finally {
		await disposeContext(recolorContext);
	}
	return witnesses;
}

async function sha256(path: string): Promise<string | null> {
	const file = Bun.file(path);
	if (!(await file.exists())) return null;
	const digest = await crypto.subtle.digest(
		"SHA-256",
		await file.arrayBuffer(),
	);
	return [...new Uint8Array(digest)]
		.map((byte) => byte.toString(16).padStart(2, "0"))
		.join("");
}

function gitRevision(): string | null {
	const result = Bun.spawnSync({ cmd: ["git", "rev-parse", "HEAD"] });
	return result.exitCode === 0
		? new TextDecoder().decode(result.stdout).trim()
		: null;
}

async function provenance() {
	const schema = await Bun.file(
		new URL("../src/transport/ui/generated/ui_schema.ts", import.meta.url),
	).text();
	const schemaHash = schema.match(/schema_blake3 = ([0-9a-f]+)/u)?.[1] ?? null;
	const baselineSchemaFile = Bun.file(
		"/tmp/t6-m1-baseline/source/packages/iyon-tui/src/transport/ui/generated/ui_schema.ts",
	);
	const baselineSchema = (await baselineSchemaFile.exists())
		? await baselineSchemaFile.text()
		: "";
	return {
		currentCommit: gitRevision(),
		bun: Bun.version,
		platform: process.platform,
		architecture: process.arch,
		measurementArtifactKind:
			perfSnapshot === undefined ? "default" : "perf-counters",
		nativeArtifact: nativeArtifact.absolutePath,
		nativeArtifactSha256: await sha256(nativeArtifact.absolutePath),
		uiSchemaBlake3: schemaHash,
		baseline: {
			status: "rejected-schema-mismatch",
			artifact: "/tmp/t6-m1-baseline/iyon-tui-native.node",
			artifactSha256: await sha256("/tmp/t6-m1-baseline/iyon-tui-native.node"),
			sourceArchiveSha256:
				"9a42664709cd367af0b97e9807925f5f7896f9f063a4cd9ae93800f178386d7b",
			sourceCommit: "1d4bf7916754f09d75c0d6e39a69340f8366f04f",
			uiSchemaBlake3:
				baselineSchema.match(/schema_blake3 = ([0-9a-f]+)/u)?.[1] ?? null,
			reason:
				"archived addon/source schema differs from current generated UI schema; loader success is not structural or semantic compatibility, and this current-source benchmark rejects external artifact overrides",
		},
	};
}

console.error("Measuring traffic witnesses");
const trafficWitnesses = await runTrafficWitnesses();
const workloadFactories: readonly [string, () => Promise<Session>][] = [
	["stable-tree-leaf-style", openStableSession],
	["wide-keyed-reorder", openKeyedMoveSession],
	["deep-local-insert-remove", openDeepSession],
	["source-width-20", () => openSourceSession(20)],
	["source-width-80", () => openSourceSession(80)],
	["source-width-160", () => openSourceSession(160)],
	["markdown-burst-immediate", () => openMarkdownSession(false)],
	["markdown-burst-smooth-native", () => openMarkdownSession(true)],
	["markdown-steady-immediate", () => openMarkdownSession(false, true)],
	["markdown-steady-smooth-native", () => openMarkdownSession(true, true)],
	["native-editor", openEditorSession],
	["native-animation", openAnimationSession],
	["resize-theme-scroll", openResizeThemeSession],
	["delayed-receipt-mount-unmount", openMountUnmountSession],
	["shared-source-duplicate-occurrence-memory", openSharedMemorySession],
];
const workloads = [];
for (const [name, factory] of workloadFactories) {
	console.error(`Measuring ${name}`);
	workloads.push(await runSession(name, await factory()));
}

const report = {
	schema: "t7-m2-performance-v2",
	provenance: await provenance(),
	configuration: {
		warmupCount,
		sampleCount,
		appendCount,
		trafficAccounting:
			"transportBytes is words+metadata+ownedContent; semanticBytes is metadata+ownedContent; Source accepted/copied bytes come from the native Source stats contract",
		p99Note:
			"p99 is exploratory-max-adjacent below 20 samples; raw samples remain authoritative",
		comparisonPolicy:
			"acceptance is synchronous at root.render/direct Source mutation; visibility/output latency is measured by the separate native barrier",
		regressionThreshold:
			"investigate repeatable p95 regression above both 15% and 0.10 ms, or any repeatable multi-millisecond regression",
		instrumentation:
			perfSnapshot === undefined
				? "default addon: JavaScript traffic and barrier timings only"
				: "perf-counters addon: JavaScript timings plus native-owned stage counters",
		memoryPolicy:
			"sourceRetainedBytes is the native retained Source allocation; sourceAcceptedBytes/sourceCopiedBytes are cumulative Source statistics; process RSS and JS heap are reported separately and are not native allocation counts",
		nativeTimingOwners: {
			frame_prepare_nanos:
				"HostInner::prepare_candidate_frame_with_backend (layout/content candidate preparation)",
			runtime_advance_nanos:
				"HostInner::advance_runtime_for_candidate (scheduler/control advancement)",
			frame_present_nanos:
				"HostInner::present_frame (physical frame submission/poll)",
			frame_commit_nanos:
				"HostInner::commit_frame (visible frame/content promotion)",
			direct_capture_nanos:
				"SceneHost::capture_direct_measurements (content capture/projection entry)",
			direct_refinement_nanos:
				"SceneHost::prepare_direct_at_with_content (final-width refinement)",
			direct_driver_layout_nanos:
				"DirectDriverHandle::layout (layout-thread handoff and response)",
			taffy_layout_nanos:
				"TaffyLayoutAdapter::layout (one intrinsic/definite layout pass)",
			taffy_layout_passes: "TaffyLayoutAdapter::layout invocation count",
			direct_paint_nanos:
				"paint_direct_layout (physical Surface construction and paint)",
		},
	},
	trafficWitnesses,
	workloads,
};
const serialized = JSON.stringify(report, null, 2);
console.log(serialized);
if (outputPath !== undefined) await Bun.write(outputPath, `${serialized}\n`);
