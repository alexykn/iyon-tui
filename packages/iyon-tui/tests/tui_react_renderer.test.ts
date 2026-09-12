import { describe, expect, test } from "bun:test";
import {
	createElement,
	Fragment,
	memo,
	type ReactNode,
	StrictMode,
	startTransition,
	useEffect,
	useLayoutEffect,
	useState,
} from "react";
import {
	StyleRef,
	StyleSelector,
	StyleSpec,
	TextBlockSource,
	TextContent,
	Theme,
} from "../src/index.ts";
import { RootContainer } from "../src/react/commit.ts";
import {
	hostCandidateCreations,
	hostConfig,
	withNativeEventPriority,
} from "../src/react/host-config.ts";
import { nativeHostForReact } from "../src/react/host-registry.ts";
import {
	Animation,
	Box,
	Column,
	Content,
	type ContentConnectorToken,
	type ContentPortToken,
	createReactRoot,
	Editor,
	Grid,
	History,
	HistoryUnit,
	type OccurrenceRef,
	Row,
	Text,
	useContentConnector,
	useContentPort,
} from "../src/react/index.ts";
import { AppHarness } from "../src/testing/index.ts";
import type { NativeTuiHostContract } from "../src/transport/native/addon.ts";
import {
	UI_OPCODES,
	UI_PROPERTIES,
} from "../src/transport/ui/generated/ui_schema.ts";

function tree(label: string, background: "red" | "blue" = "red") {
	return createElement(
		Column,
		{ width: "fill" },
		createElement(
			Box,
			{ background: { type: "named", value: background } },
			createElement(Text, {}, label),
		),
	);
}

function assertLayoutProperty(
	words: Uint32Array | undefined,
	expected: number,
): void {
	if (words === undefined)
		throw new Error("native commit did not produce a word lane");
	const stateStart = 16 + (words[7] ?? 0);
	const stateEnd = stateStart + (words[8] ?? 0);
	let found = false;
	for (let cursor = stateStart; cursor < stateEnd; ) {
		const count = words[cursor + 1] ?? 0;
		if (words[cursor] === 0x10 && words[cursor + 6] === UI_PROPERTIES.layout) {
			expect(words[cursor + 7]).toBe(expected);
			found = true;
		}
		if (count < 2)
			throw new Error("native commit contained an invalid record width");
		cursor += count;
	}
	expect(found).toBe(true);
}

function commitRecords(words: Uint32Array): Array<{
	readonly opcode: number;
	readonly operands: readonly number[];
}> {
	const records: Array<{
		readonly opcode: number;
		readonly operands: readonly number[];
	}> = [];
	let cursor = 16;
	for (let section = 0; section < 5; section += 1) {
		const end = cursor + (words[7 + section] ?? 0);
		while (cursor < end) {
			const width = words[cursor + 1] ?? 0;
			if (width < 2)
				throw new Error("native commit contained an invalid record");
			records.push({
				opcode: words[cursor] ?? 0,
				operands: Array.from(words.slice(cursor + 2, cursor + width)),
			});
			cursor += width;
		}
	}
	return records;
}

function commitOpcodes(words: Uint32Array): number[] {
	return commitRecords(words).map((record) => record.opcode);
}

function acknowledgedHandle(
	acknowledgement: Uint32Array,
	ordinal: number,
): number[] {
	return Array.from(
		acknowledgement.slice(8 + (ordinal - 1) * 4, 12 + (ordinal - 1) * 4),
	);
}

function selectedConnector(
	words: Uint32Array,
):
	| { readonly port: readonly number[]; readonly connector: readonly number[] }
	| undefined {
	const record = commitRecords(words).find(
		(candidate) => candidate.opcode === UI_OPCODES.selectConnector,
	);
	if (
		record === undefined ||
		record.operands.slice(4, 8).every((word) => word === 0)
	)
		return undefined;
	return {
		port: record.operands.slice(0, 4),
		connector: record.operands.slice(4, 8),
	};
}

function required<T>(value: T | undefined, message: string): T {
	if (value === undefined) throw new Error(message);
	return value;
}

async function waitForHostIdle(tui: AppHarness): Promise<void> {
	const deadline = performance.now() + 1_000;
	for (;;) {
		tui.flush();
		const epochs = tui.epochs();
		if (epochs.pending_epoch === epochs.committed_epoch) return;
		if (performance.now() >= deadline)
			throw new Error(`native host stalled: ${JSON.stringify(epochs)}`);
		await Bun.sleep(1);
	}
}

async function waitForNativeCommits(
	getCount: () => number,
	minimum: number,
): Promise<void> {
	const deadline = Date.now() + 500;
	while (getCount() < minimum) {
		if (Date.now() >= deadline)
			throw new Error(`timed out waiting for ${minimum} native commits`);
		await new Promise((resolve) => setTimeout(resolve, 5));
	}
}

async function waitForCandidateYield(
	before: number,
	getCandidates: () => number,
	getNativeCommits: () => number,
	expectedNativeCommits: number,
): Promise<void> {
	for (let attempt = 0; attempt < 100; attempt += 1) {
		await new Promise<void>((resolve) => setTimeout(resolve, 0));
		const nativeCommits = getNativeCommits();
		if (nativeCommits > expectedNativeCommits)
			throw new Error("native commit overtook candidate-yield observation");
		if (getCandidates() > before && nativeCommits === expectedNativeCommits)
			return;
	}
	throw new Error(
		"the concurrent render did not yield before its native commit",
	);
}

describe("T3 React mutation renderer", () => {
	test("accepts literal payloads larger than the JavaScript argument limit", async () => {
		const tui = await AppHarness.open();
		const root = createReactRoot(tui);
		const literal = "x".repeat(1_000_000);
		try {
			const mounted = await root.render(createElement(Text, {}, literal));
			expect(mounted.accepted).toBe(true);
			const unchanged = await root.render(createElement(Text, {}, literal));
			expect(unchanged.revision).toBe(mounted.revision);
		} finally {
			root.close();
			tui.close();
		}
	});

	test("mounts through the actual staged native commit boundary", async () => {
		const tui = await AppHarness.open({ width: 32, height: 8 });
		const root = createReactRoot(tui);
		try {
			const result = await root.render(tree("ready"));
			expect(result).toEqual({ revision: 1, accepted: true });
			expect(root.faulted).toBe(false);
		} finally {
			await root.unmount();
			tui.close();
		}
	});

	test("normalizes raw JSX text, keyed placement, and property updates", async () => {
		const tui = await AppHarness.open({ width: 32, height: 8 });
		const root = createReactRoot(tui);
		const keyed = (items: readonly string[]) =>
			createElement(
				Box,
				{},
				items.map((item) => createElement(Text, { key: item }, item)),
			);
		try {
			expect((await root.render(keyed(["a", "b", "c"]))).revision).toBe(1);
			expect((await root.render(keyed(["c", "b", "a"]))).revision).toBe(2);
			expect((await root.render(tree("changed", "blue"))).revision).toBe(3);
		} finally {
			await root.unmount();
			tui.close();
		}
	});

	test("preserves ordered identity for prepend, first move, and nested insertion", async () => {
		const tui = await AppHarness.open({ width: 32, height: 8 });
		const root = createReactRoot(tui);
		const refs = new Map<string, object>();
		const keyed = (items: readonly string[]) =>
			createElement(
				Box,
				{},
				items.map((item) =>
					createElement(
						Text,
						{
							key: item,
							ref: (value) => {
								if (typeof value === "object" && value !== null)
									refs.set(item, value);
							},
						},
						item,
					),
				),
			);
		try {
			expect((await root.render(keyed(["a", "b"]))).revision).toBe(1);
			const a = refs.get("a");
			expect(a).toBeDefined();
			expect((await root.render(keyed(["x", "a", "b"]))).revision).toBe(2);
			expect((await root.render(keyed(["a", "x", "b"]))).revision).toBe(3);
			expect(refs.get("a")).toBe(a);
			expect(
				(
					await root.render(
						createElement(
							Box,
							{},
							createElement(Box, {}, createElement(Text, {}, "nested")),
						),
					)
				).revision,
			).toBe(4);
		} finally {
			await root.unmount();
			tui.close();
		}
	});

	test("identical output and callback identity-only changes do not call native UI", async () => {
		const tui = await AppHarness.open({ width: 32, height: 8 });
		const root = createReactRoot(tui);
		const callback = () => {};
		const host = nativeHostForReact(tui) as NativeTuiHostContract | undefined;
		if (host === undefined)
			throw new Error("native host association is unavailable");
		let nativeCalls = 0;
		const originalCommit = host.commitUiV1.bind(host);
		host.commitUiV1 = (...args) => {
			nativeCalls += 1;
			return originalCommit(...args);
		};
		try {
			expect(
				(await root.render(createElement(Box, { onPress: callback }, "same")))
					.revision,
			).toBe(1);
			expect(nativeCalls).toBe(1);
			expect(
				(await root.render(createElement(Box, { onPress: callback }, "same")))
					.revision,
			).toBe(1);
			expect(nativeCalls).toBe(1);
			expect(
				(await root.render(createElement(Box, { onPress: () => {} }, "same")))
					.revision,
			).toBe(1);
			expect(nativeCalls).toBe(1);
			expect((await root.render(createElement(Box, {}, "same"))).revision).toBe(
				2,
			);
			expect(nativeCalls).toBe(2);
			const firstPadding = { top: 1, right: 2, bottom: 3, left: 4 };
			const equivalentPadding = { left: 4, bottom: 3, right: 2, top: 1 };
			expect(
				(
					await root.render(
						createElement(Box, { padding: firstPadding }, "same"),
					)
				).revision,
			).toBe(3);
			expect(nativeCalls).toBe(3);
			expect(
				(
					await root.render(
						createElement(Box, { padding: equivalentPadding }, "same"),
					)
				).revision,
			).toBe(3);
			expect(nativeCalls).toBe(3);
		} finally {
			await root.unmount();
			tui.close();
		}
	});

	test("post-mount traffic carries only the changed semantic lane", async () => {
		const tui = await AppHarness.open({ width: 40, height: 10 });
		const root = createReactRoot(tui);
		const host = required(
			nativeHostForReact(tui) as NativeTuiHostContract | undefined,
			"native host association is unavailable",
		);
		const commits: Array<{
			readonly words: Uint32Array;
			readonly metadata: Uint8Array;
			readonly content: Uint8Array;
		}> = [];
		const originalCommit = host.commitUiV1.bind(host);
		host.commitUiV1 = (words, metadata, content, sources) => {
			commits.push({ words, metadata, content });
			return originalCommit(words, metadata, content, sources);
		};
		const resetTraffic = () => {
			commits.length = 0;
		};
		const opcodes = () => commits.flatMap(({ words }) => commitOpcodes(words));
		const trafficBytes = () =>
			commits.reduce(
				(total, { words, metadata, content }) =>
					total + words.byteLength + metadata.byteLength + content.byteLength,
				0,
			);
		const callback = () => {};
		const keyed = (
			background: "red" | "blue",
			items: readonly (string | readonly [string, string])[],
			handler: () => void,
		) =>
			createElement(
				Box,
				{ background: { type: "named", value: background }, onPress: handler },
				items.map((item) => {
					const key = typeof item === "string" ? item : item[0];
					const text = typeof item === "string" ? item : item[1];
					return createElement(Text, { key }, text);
				}),
			);
		try {
			await root.render(keyed("red", ["a", "b", "c", "d"], callback));
			await root.whenVisible();
			resetTraffic();
			await root.render(keyed("red", ["a", "b", "c", "d"], callback));
			expect(opcodes()).toEqual([]);
			expect(trafficBytes()).toBe(0);

			resetTraffic();
			await root.render(keyed("red", ["a", "b", "c", "d"], () => {}));
			expect(opcodes()).toEqual([]);
			expect(trafficBytes()).toBe(0);

			resetTraffic();
			await root.render(keyed("blue", ["a", "b", "c", "d"], callback));
			expect(opcodes()).toEqual([UI_OPCODES.setDeclared]);
			expect(commits[0]?.metadata.byteLength).toBe(0);
			expect(commits[0]?.content.byteLength).toBe(0);

			resetTraffic();
			await root.render(
				keyed("blue", [["a", "a"], ["b", "changed"], "c", "d"], callback),
			);
			expect(opcodes()).toEqual([UI_OPCODES.replaceLiteral]);
			expect(commits[0]?.content.byteLength).toBeGreaterThan(0);
			expect(commits[0]?.metadata.byteLength).toBe(0);

			resetTraffic();
			await root.render(
				keyed("blue", ["d", "c", ["b", "changed"], "a"], callback),
			);
			expect(opcodes().length).toBeGreaterThan(0);
			expect(
				opcodes().every((opcode) => opcode === UI_OPCODES.insertBefore),
			).toBe(true);
			expect(commits[0]?.metadata.byteLength).toBe(0);
			expect(commits[0]?.content.byteLength).toBe(0);
		} finally {
			await root.unmount();
			root.close();
			tui.close();
		}
	});

	test("public style states lower through the theme and explicit ref overrides", async () => {
		const tui = await AppHarness.open({ width: 24, height: 6 });
		tui.setTheme(
			Theme.new()
				.withStyle(
					"severity",
					new StyleSpec().foreground({ type: "named", value: "red" }),
				)
				.withStyleVariant(
					"severity",
					StyleSelector.state("severity", "error"),
					new StyleSpec().foreground({ type: "named", value: "green" }),
				)
				.withStyleVariant(
					"severity",
					StyleSelector.state("severity", "warning"),
					new StyleSpec().foreground({ type: "named", value: "yellow" }),
				),
		);
		const root = createReactRoot(tui);
		let ref: OccurrenceRef | undefined;
		let nativeCalls = 0;
		const host = required(
			nativeHostForReact(tui) as NativeTuiHostContract | undefined,
			"native host association is unavailable",
		);
		const originalCommit = host.commitUiV1.bind(host);
		host.commitUiV1 = (...args) => {
			nativeCalls += 1;
			return originalCommit(...args);
		};
		try {
			await root.render(
				createElement(
					Box,
					{
						style: StyleRef.theme("severity"),
						styleStates: { severity: "info" },
						ref: (value) => {
							if (value !== null) ref = value;
						},
					},
					"state",
				),
			);
			await root.whenVisible();
			const row = tui.screenRows().findIndex((line) => line.includes("state"));
			const column = tui.cellXOfText(row, "state");
			if (row < 0 || column === null || ref === undefined)
				throw new Error("style-state text/ref was not published");
			const styleRef = ref;
			expect(() => styleRef.setStyleState("severity", "")).toThrow(
				"style state values must be nonempty NUL-free strings",
			);
			expect(() => styleRef.clearStyleState("\0")).toThrow(
				"style state keys must be nonempty and NUL-free",
			);
			expect(tui.styleAt(row, column).foreground).toBe("Red");
			const acceptedCalls = nativeCalls;
			await root.render(
				createElement(
					Box,
					{
						style: StyleRef.theme("severity"),
						styleStates: { severity: "info" },
					},
					"state",
				),
			);
			expect(nativeCalls).toBe(acceptedCalls);
			expect(styleRef.setStyleState("severity", "warning").accepted).toBe(true);
			await root.whenVisible();
			expect(tui.styleAt(row, column).foreground).toBe("Yellow");
			const changedCalls = nativeCalls;
			await root.render(
				createElement(
					Box,
					{
						style: StyleRef.theme("severity"),
						styleStates: { severity: "error" },
					},
					"state",
				),
			);
			expect(nativeCalls).toBeGreaterThan(changedCalls);
			expect(tui.styleAt(row, column).foreground).toBe("Yellow");
			expect(styleRef.clearStyleState("severity").accepted).toBe(true);
			await root.whenVisible();
			expect(tui.styleAt(row, column).foreground).toBe("Green");
		} finally {
			await root.unmount();
			root.close();
			tui.close();
		}
	});

	test("public alignment accepts neutral axes, maps row bottom, and rejects horizontal center", async () => {
		const verticalTui = await AppHarness.open({ width: 20, height: 6 });
		const verticalRoot = createReactRoot(verticalTui);
		try {
			await verticalRoot.render(createElement(Box, {}, "neutral"));
			await verticalRoot.whenVisible();
			const defaultRows = verticalTui.screenRows();
			await verticalRoot.render(
				createElement(Box, { alignment: {} }, "neutral"),
			);
			await verticalRoot.whenVisible();
			expect(verticalTui.screenRows()).toEqual(defaultRows);
			await verticalRoot.render(
				createElement(
					Row,
					{ height: "fill", alignment: { vertical: "bottom" } },
					createElement(Text, {}, "row"),
				),
			);
			await verticalRoot.whenVisible();
			const rows = verticalTui.screenRows();
			expect(rows[rows.length - 1]).toContain("row");
		} finally {
			verticalRoot.close();
			verticalTui.close();
		}

		const horizontalTui = await AppHarness.open({ width: 20, height: 6 });
		const horizontalRoot = createReactRoot(horizontalTui);
		try {
			await horizontalRoot.render(
				createElement(
					Row,
					{ alignment: { horizontal: "center" } },
					createElement(Text, {}, "row"),
				),
			);
			await expect(horizontalRoot.whenVisible()).rejects.toThrow(
				"alignment.horizontal is unsupported in the M1 terminal adapter",
			);
		} finally {
			horizontalRoot.close();
			horizontalTui.close();
		}
	});

	test("ordinary React state updates schedule accepted native content", async () => {
		const tui = await AppHarness.open();
		const host = required(
			nativeHostForReact(tui) as NativeTuiHostContract | undefined,
			"native host association is unavailable",
		);
		const commits: Uint32Array[] = [];
		const originalCommit = host.commitUiV1.bind(host);
		host.commitUiV1 = (words, ...args) => {
			const acknowledgement = originalCommit(words, ...args);
			commits.push(words);
			return acknowledgement;
		};
		let setValue: ((value: string) => void) | undefined;
		function ScheduledText() {
			const [value, set] = useState("before");
			setValue = set;
			useLayoutEffect(() => {
				if (value === "before") set("layout");
			}, [value]);
			useEffect(() => {
				if (value === "layout") set("passive");
			}, [value]);
			return createElement(Text, {}, value);
		}
		const root = createReactRoot(tui);
		try {
			await root.render(createElement(ScheduledText));
			await waitForNativeCommits(() => commits.length, 3);
			const update = required(setValue, "state setter was not published");
			update("external");
			await waitForNativeCommits(() => commits.length, 4);
			expect(commitOpcodes(commits.at(-1) as Uint32Array)).toContain(
				UI_OPCODES.replaceLiteral,
			);
		} finally {
			await root.unmount();
			tui.close();
		}
	});

	test("acceptance and presentation barriers complete on the native drain", async () => {
		const tui = await AppHarness.open();
		const root = createReactRoot(tui);
		try {
			const result = await root.render(createElement(Text, {}, "accepted"));
			await root.whenVisible(result.revision);
			expect(tui.screenRows().some((row) => row.includes("accepted"))).toBe(
				true,
			);
		} finally {
			await root.unmount();
			tui.close();
		}
	});

	test("native frame realizes React content and editor controls", async () => {
		const tui = await AppHarness.open({ width: 24, height: 6 });
		const source = TextBlockSource.create();
		source.replace("source one");
		function SourceContent() {
			const port = useContentPort();
			const connector = useContentConnector({ port, source });
			return createElement(Content, { port: connector });
		}
		const root = createReactRoot(tui);
		try {
			await root.render(
				createElement(
					Column,
					{},
					createElement(Text, {}, "literal"),
					createElement(SourceContent),
					createElement(Editor, { defaultValue: "edit" }),
				),
			);
			await root.whenContentVisible();
			expect(tui.screenRows().some((row) => row.includes("literal"))).toBe(
				true,
			);
			expect(tui.screenRows().some((row) => row.includes("source one"))).toBe(
				true,
			);
			tui.pressKey("x");
			// Native input advances work, not the already-visible UI revision.
			const editDeadline = performance.now() + 1_000;
			while (
				!tui.screenRows().some((row) => row.includes("editx")) &&
				performance.now() < editDeadline
			) {
				await Bun.sleep(1);
			}
			expect(tui.screenRows().some((row) => row.includes("editx"))).toBe(true);
			source.replace("source two");
			await root.whenContentVisible();
			expect(tui.screenRows().some((row) => row.includes("source two"))).toBe(
				true,
			);
		} finally {
			await root.unmount();
			tui.close();
			source.dispose();
		}
	});

	test("native editor events dispatch captured callbacks after the host lock", async () => {
		const tui = await AppHarness.open({ width: 24, height: 6 });
		const events: string[] = [];
		let resolveDelivered: (() => void) | undefined;
		const delivered = new Promise<void>((resolve) => {
			resolveDelivered = resolve;
		});
		const root = createReactRoot(tui);
		try {
			await root.render(
				createElement(Editor, {
					defaultValue: "edit",
					onInput: () => events.push("input"),
					onEdit: () => events.push("edit"),
					onChange: () => {
						events.push("change");
						if (events.includes("input") && events.includes("edit"))
							resolveDelivered?.();
					},
					onSelectionChange: () => events.push("selection"),
				}),
			);
			await root.whenVisible();
			tui.pressKey("x");
			await delivered;
			expect(events).toContain("input");
			expect(events).toContain("edit");
			expect(events).toContain("change");
		} finally {
			await root.unmount();
			tui.close();
		}
	});

	test("native event priorities restore after callback failure", () => {
		expect(
			withNativeEventPriority("discrete", () =>
				hostConfig.getCurrentUpdatePriority(),
			),
		).toBe(2);
		expect(hostConfig.getCurrentUpdatePriority()).toBe(0);
		expect(() =>
			withNativeEventPriority("continuous", () => {
				expect(hostConfig.getCurrentUpdatePriority()).toBe(8);
				throw new Error("priority callback failed");
			}),
		).toThrow("priority callback failed");
		expect(hostConfig.getCurrentUpdatePriority()).toBe(0);
	});

	test("stale native event generations increment the diagnostic count", async () => {
		const tui = await AppHarness.open({ width: 24, height: 6 });
		const host = required(
			nativeHostForReact(tui) as NativeTuiHostContract | undefined,
			"native host association is unavailable",
		);
		const container = new RootContainer(host);
		try {
			container.coordinator.dispatchNativeEvents([
				{
					host_namespace: host.uiNamespace(),
					slot: 999,
					generation: 7,
					kind: 1,
					mask: 2,
					text: "stale",
					cursor_bytes: 0,
					revision: 1,
				},
			]);
			expect(container.coordinator.staleNativeEvents).toBe(1);
		} finally {
			tui.close();
		}
	});

	test("native event lane delivers edits without a presentation barrier", async () => {
		const tui = await AppHarness.open({ width: 24, height: 6 });
		const seen: string[] = [];
		const reported: unknown[] = [];
		let resolveDelivered: (() => void) | undefined;
		const delivered = new Promise<void>((resolve) => {
			resolveDelivered = resolve;
		});
		const previousReportError = (
			globalThis as unknown as { reportError?: (error: unknown) => void }
		).reportError;
		(
			globalThis as unknown as { reportError?: (error: unknown) => void }
		).reportError = (error) => reported.push(error);
		const root = createReactRoot(tui);
		try {
			await root.render(
				createElement(Editor, {
					defaultValue: "edit",
					onInput: (event) => {
						expect(Object.isFrozen(event)).toBe(true);
						expect(Object.isFrozen(event.target)).toBe(true);
						seen.push(event.text);
						if (seen.length === 2) resolveDelivered?.();
						if (event.text === "editx")
							throw new Error("input observer failed");
					},
				}),
			);
			await root.whenVisible();
			// This lane is intentionally independent from whenVisible().
			tui.pressKey("x");
			tui.pressKey("y");
			await delivered;
			expect(seen).toEqual(["editx", "editxy"]);
			expect(reported).toHaveLength(1);
		} finally {
			root.close();
			tui.close();
			(
				globalThis as unknown as { reportError?: (error: unknown) => void }
			).reportError = previousReportError;
		}
	});

	test("native key payload keeps Key identity and decoded submit semantics", async () => {
		const tui = await AppHarness.open({ width: 24, height: 6 });
		const events: string[] = [];
		let resolveShift: (() => void) | undefined;
		const shiftDelivered = new Promise<void>((resolve) => {
			resolveShift = resolve;
		});
		let resolveSubmit: (() => void) | undefined;
		const submitDelivered = new Promise<void>((resolve) => {
			resolveSubmit = resolve;
		});
		const root = createReactRoot(tui);
		try {
			await root.render(
				createElement(Editor, {
					multiline: true,
					defaultValue: "edit",
					onEdit: (event) => {
						events.push(`edit:${event.key}`);
						if (event.text.endsWith("\n")) resolveShift?.();
					},
					onPress: (event) => events.push(`press:${event.key}`),
					onSubmit: () => {
						events.push("submit");
						resolveSubmit?.();
					},
				}),
			);
			await root.whenVisible();
			tui.pressKey("Enter", ["Shift"]);
			await shiftDelivered;
			expect(events).toEqual(["edit:Enter"]);
			tui.pressKey("Enter");
			await submitDelivered;
			expect(events).toEqual(["edit:Enter", "press:Enter", "submit"]);
		} finally {
			root.close();
			tui.close();
		}
	});

	test("callback-triggered root close stops the native event lane", async () => {
		const tui = await AppHarness.open({ width: 24, height: 6 });
		const calls: string[] = [];
		let resolveClosed: (() => void) | undefined;
		const closed = new Promise<void>((resolve) => {
			resolveClosed = resolve;
		});
		const root = createReactRoot(tui);
		try {
			await root.render(
				createElement(Editor, {
					defaultValue: "edit",
					onInput: (event) => {
						calls.push(event.text);
						root.close();
						resolveClosed?.();
					},
				}),
			);
			await root.whenVisible();
			tui.pressKey("x");
			await closed;
			expect(calls).toEqual(["editx"]);
			await expect(root.render(createElement(Editor, {}))).rejects.toThrow(
				"React root is closed",
			);
		} finally {
			root.close();
			tui.close();
		}
	});

	test("native event admission rejects byte overflow before editor mutation", async () => {
		const tui = await AppHarness.open({ width: 24, height: 6 });
		const host = required(
			nativeHostForReact(tui) as NativeTuiHostContract | undefined,
			"native host association is unavailable",
		);
		// Keep the one root event waiter from consuming the queue so this test
		// exercises native admission rather than a racing JS drain.
		host.waitForUiEvents = () => new Promise(() => {});
		host.setUiEventQueueLimits(4096, 4);
		const root = createReactRoot(tui);
		try {
			await root.render(
				createElement(Editor, { defaultValue: "edit", onInput: () => {} }),
			);
			await root.whenVisible();
			expect(() => tui.pressKey("x")).toThrow("one native UI event batch");
			expect(host.screenRows().some((row) => row.includes("edit"))).toBe(true);
			expect(host.drainUiEvents()).toEqual([]);
		} finally {
			root.close();
			tui.close();
		}
	});

	test("native event admission preserves queued order across record backpressure", async () => {
		const tui = await AppHarness.open({ width: 24, height: 6 });
		const host = required(
			nativeHostForReact(tui) as NativeTuiHostContract | undefined,
			"native host association is unavailable",
		);
		host.waitForUiEvents = () => new Promise(() => {});
		host.setUiEventQueueLimits(1, 1024);
		const root = createReactRoot(tui);
		try {
			await root.render(
				createElement(Editor, { defaultValue: "edit", onInput: () => {} }),
			);
			await root.whenVisible();
			tui.pressKey("x");
			expect(() => tui.pressKey("y")).toThrow("native UI event queue is full");
			// The accepted native event is authoritative for edit state;
			// asynchronous paint is covered by the frame-realization test.
			const first = host.drainUiEvents();
			expect(first).toHaveLength(1);
			expect(first[0]?.text).toBe("editx");
			tui.pressKey("y");
			const second = host.drainUiEvents();
			expect(second).toHaveLength(1);
			expect(second[0]?.text).toBe("editxy");
		} finally {
			root.close();
			tui.close();
		}
	});

	test("native cursor and paste events preserve editor revision filters", async () => {
		const tui = await AppHarness.open({ width: 24, height: 6 });
		const events: string[] = [];
		let resolveCursor: (() => void) | undefined;
		const cursorDelivered = new Promise<void>((resolve) => {
			resolveCursor = resolve;
		});
		let resolvePaste: (() => void) | undefined;
		const pasteDelivered = new Promise<void>((resolve) => {
			resolvePaste = resolve;
		});
		const root = createReactRoot(tui);
		try {
			await root.render(
				createElement(Editor, {
					defaultValue: "edit",
					onEdit: (event) => {
						const value = `edit:${event.text}:${event.cursorBytes}`;
						events.push(value);
						if (value === "edit:edit:3") resolveCursor?.();
						if (value === "edit:edi!t:4") resolvePaste?.();
					},
					onChange: (event) => {
						events.push(`change:${event.text}`);
						if (event.text === "edi!t") resolvePaste?.();
					},
					onSelectionChange: (event) => {
						events.push(`selection:${event.cursorBytes}`);
						if (event.cursorBytes === 3) resolveCursor?.();
					},
				}),
			);
			await root.whenVisible();
			tui.pressKey("Left");
			await cursorDelivered;
			expect(events).toContain("edit:edit:3");
			expect(events).toContain("selection:3");
			expect(events).not.toContain("change:edit");
			tui.paste("!");
			await pasteDelivered;
			expect(events).toContain("edit:edi!t:4");
			expect(events).toContain("change:edi!t");
		} finally {
			root.close();
			tui.close();
		}
	});

	test("failed native content switch retains the prior confirmed projection", async () => {
		const tui = await AppHarness.open({ width: 24, height: 6 });
		const host = required(
			nativeHostForReact(tui) as NativeTuiHostContract | undefined,
			"native host association is unavailable",
		);
		const sourceA = TextBlockSource.create();
		const sourceB = TextBlockSource.create();
		sourceA.replace("confirmed-a");
		sourceB.replace("candidate-b");
		function DynamicContent({ source }: { readonly source: TextBlockSource }) {
			const port = useContentPort();
			const connector = useContentConnector({ port, source });
			return createElement(Content, { port: connector });
		}
		const root = createReactRoot(tui);
		try {
			await root.render(createElement(DynamicContent, { source: sourceA }));
			await root.whenContentVisible();
			expect(host.screenRows().some((row) => row.includes("confirmed-a"))).toBe(
				true,
			);
			// Arm the existing native failure seam before React accepts the
			// replacement. The all-zero private handle means "next newly
			// prepared UI Connector"; this avoids racing native auto-presentation.
			host.failUiConnectorForTest(
				[0, 0, 0, 0],
				"injected candidate projection failure",
			);
			await root.render(createElement(DynamicContent, { source: sourceB }));
			await expect(root.whenContentVisible()).rejects.toBeInstanceOf(Error);
			expect(host.screenRows().some((row) => row.includes("confirmed-a"))).toBe(
				true,
			);
		} finally {
			root.close();
			tui.close();
			sourceA.dispose();
			sourceB.dispose();
		}
	});

	test("animation identity advances on the native deadline without React work", async () => {
		const tui = await AppHarness.open({ width: 24, height: 6 });
		const host = required(
			nativeHostForReact(tui) as NativeTuiHostContract | undefined,
			"native host association is unavailable",
		);
		let nativeCommits = 0;
		const originalCommit = host.commitUiV1.bind(host);
		host.commitUiV1 = (...args) => {
			nativeCommits += 1;
			return originalCommit(...args);
		};
		const root = createReactRoot(tui);
		try {
			await root.render(
				createElement(
					Animation,
					{ intervalMs: 10 },
					createElement(Box, {}, createElement(Text, {}, "frame-a")),
					createElement(Box, {}, createElement(Text, {}, "frame-b")),
				),
			);
			await root.whenContentVisible();
			expect(tui.screenRows().some((row) => row.includes("frame-a"))).toBe(
				true,
			);
			const acceptedCommits = nativeCommits;
			// A completed native sync must be consumed before the next tick:
			// these changes deliberately share the same accepted UI revision.
			for (const expected of ["frame-b", "frame-a", "frame-b"]) {
				tui.advance(20);
				const deadline = performance.now() + 1_000;
				while (
					!tui.screenRows().some((row) => row.includes(expected)) &&
					performance.now() < deadline
				) {
					await Bun.sleep(1);
				}
				expect(nativeCommits).toBe(acceptedCommits);
				expect(tui.screenRows().some((row) => row.includes(expected))).toBe(
					true,
				);
			}
			// A burst may coalesce native ticks, but all accepted host work must
			// still settle even while the previous synchronization is in flight.
			for (let tick = 0; tick < 8; tick += 1) tui.advance(20);
			await waitForHostIdle(tui);
			expect(nativeCommits).toBe(acceptedCommits);
		} finally {
			await root.unmount();
			tui.close();
		}
	});

	test("public refs publish overrides and resolve confirmed ordinary geometry", async () => {
		const tui = await AppHarness.open();
		const root = createReactRoot(tui);
		let ref:
			| {
					setOverride(
						property: string,
						value: unknown,
					): {
						readonly revision: number;
						readonly accepted: true;
					};
					clearOverride(property: string): {
						readonly revision: number;
						readonly accepted: true;
					};
					focus(): void;
					visibleGeometry(): Promise<unknown>;
			  }
			| undefined;
		try {
			await root.render(
				createElement(
					Box,
					{
						ref: (value) => {
							if (value !== null) ref = value as NonNullable<typeof ref>;
						},
					},
					"box",
				),
			);
			await root.whenVisible();
			if (ref === undefined) throw new Error("ref was not published");
			const instance = ref;
			expect(
				instance.setOverride("background", { type: "named", value: "red" })
					.accepted,
			).toBe(true);
			expect(instance.clearOverride("background").accepted).toBe(true);
			expect(() => instance.focus()).toThrow("FOCUS_UNSUPPORTED");
			// The Body's Flex cross-axis stretch applies to an auto-width Box.
			// The ref reports the allocated Box, not its fitted text product.
			expect(await instance.visibleGeometry()).toEqual({
				x: 0,
				y: 23,
				width: 80,
				height: 1,
			});
		} finally {
			await root.unmount();
			tui.close();
		}
	});

	test("native-control refs focus and reject hidden or retired occurrences", async () => {
		const tui = await AppHarness.open({ width: 24, height: 6 });
		const root = createReactRoot(tui);
		let editorRef:
			| {
					focus(): void;
					visibleGeometry(): Promise<unknown>;
			  }
			| undefined;
		let hiddenRef:
			| {
					focus(): void;
					visibleGeometry(): Promise<unknown>;
			  }
			| undefined;
		try {
			await root.render(
				createElement(
					Column,
					{},
					createElement(Editor, {
						defaultValue: "visible",
						ref: (value) => {
							if (value !== null)
								editorRef = value as NonNullable<typeof editorRef>;
						},
					}),
					createElement(
						Box,
						{ hidden: true },
						createElement(Editor, {
							ref: (value) => {
								if (value !== null)
									hiddenRef = value as NonNullable<typeof hiddenRef>;
							},
						}),
					),
				),
			);
			await root.whenVisible();
			if (editorRef === undefined || hiddenRef === undefined)
				throw new Error("native control refs were not published");
			const editor = editorRef;
			const hidden = hiddenRef;
			editor.focus();
			expect(await editor.visibleGeometry()).toMatchObject({
				x: 0,
				width: 24,
				height: 1,
			});
			expect(await hidden.visibleGeometry()).toBeNull();
			expect(() => hidden.focus()).toThrow("FOCUS_UNAVAILABLE");
			await root.unmount();
			await expect(editor.visibleGeometry()).rejects.toThrow("accepted");
		} finally {
			root.close();
			tui.close();
		}
	});

	test("React History units publish ordered typed roots", async () => {
		const tui = await AppHarness.open({ width: 24, height: 6 });
		const root = createReactRoot(tui);
		try {
			await root.render(
				createElement(
					History,
					{},
					createElement(HistoryUnit, {}, "first"),
					createElement(
						HistoryUnit,
						{ flowBoundary: "attachToPrevious" },
						"second",
					),
				),
			);
			await root.whenVisible();
			expect(
				tui
					.screenRows()
					.filter((row) => row.trim() !== "")
					.map((row) => row.trim()),
			).toEqual(["first", "second"]);
		} finally {
			root.close();
			tui.close();
		}
	});

	test("React History units support live replacement, discard, and freeze", async () => {
		const tui = await AppHarness.open({ width: 24, height: 6 });
		const root = createReactRoot(tui);
		let firstRef: { historyIdentity(): number | string } | undefined;
		try {
			const renderHistory = (first: string): ReactNode =>
				createElement(
					History,
					{},
					createElement(
						HistoryUnit,
						{
							ref: (value) => {
								if (value !== null)
									firstRef = value as NonNullable<typeof firstRef>;
							},
						},
						createElement(Editor, { value: first }),
					),
					createElement(
						HistoryUnit,
						{},
						createElement(Editor, { defaultValue: "tail" }),
					),
				);
			await root.render(renderHistory("live"));
			await root.whenVisible();
			if (firstRef === undefined)
				throw new Error("History unit ref was not published");
			const firstIdentity = firstRef.historyIdentity();
			expect(tui.screenRows().some((row) => row.includes("live"))).toBe(true);
			expect(tui.screenRows().some((row) => row.includes("tail"))).toBe(true);

			await root.render(renderHistory("replacement"));
			await root.whenVisible();
			expect(firstRef.historyIdentity()).toBe(firstIdentity);
			expect(tui.screenRows().some((row) => row.includes("replacement"))).toBe(
				true,
			);

			await root.render(
				createElement(
					History,
					{},
					createElement(
						HistoryUnit,
						{},
						createElement(Editor, { value: "replacement" }),
					),
				),
			);
			await root.whenVisible();
			expect(tui.screenRows().some((row) => row.includes("replacement"))).toBe(
				true,
			);
			expect(tui.screenRows().some((row) => row.includes("tail"))).toBe(false);
		} finally {
			root.close();
			tui.close();
		}

		const staticTui = await AppHarness.open({ width: 24, height: 6 });
		const staticRoot = createReactRoot(staticTui);
		try {
			await staticRoot.render(
				createElement(
					History,
					{},
					createElement(HistoryUnit, { action: "freeze" }, "frozen"),
				),
			);
			await staticRoot.whenVisible();
			expect(staticTui.screenRows().some((row) => row.includes("frozen"))).toBe(
				true,
			);
		} finally {
			staticRoot.close();
			staticTui.close();
		}

		const freezeTui = await AppHarness.open({ width: 24, height: 6 });
		const freezeRoot = createReactRoot(freezeTui);
		try {
			await expect(
				freezeRoot.render(
					createElement(
						History,
						{},
						createElement(
							HistoryUnit,
							{ action: "freeze" },
							createElement(Editor, { defaultValue: "live control" }),
						),
					),
				),
			).rejects.toThrow("native UI commit rejected");
		} finally {
			freezeRoot.close();
			freezeTui.close();
		}
	});

	test("React History freeze installs the accepted final recipe and keyed removal discards the tail", async () => {
		const tui = await AppHarness.open({ width: 24, height: 4 });
		const root = createReactRoot(tui);
		try {
			await root.render(
				createElement(
					History,
					{},
					createElement(HistoryUnit, {}, "transient"),
					createElement(HistoryUnit, {}, "tail"),
					createElement(HistoryUnit, {}, "third"),
					createElement(HistoryUnit, {}, "fourth"),
				),
			);
			await root.whenVisible();
			await root.render(
				createElement(
					History,
					{},
					createElement(HistoryUnit, { action: "freeze" }, "final"),
					createElement(HistoryUnit, {}, "tail"),
					createElement(HistoryUnit, {}, "third"),
					createElement(HistoryUnit, {}, "fourth"),
				),
			);
			await root.whenVisible();
			expect(tui.screenRows().join("\n")).toContain("final");
			expect(tui.screenRows().join("\n")).toContain("tail");

			await root.render(
				createElement(
					History,
					{},
					createElement(HistoryUnit, { action: "freeze" }, "final"),
					createElement(HistoryUnit, {}, "tail"),
					createElement(HistoryUnit, {}, "third"),
				),
			);
			await root.whenVisible();
			expect(tui.screenRows().join("\n")).not.toContain("fourth");
		} finally {
			root.close();
			tui.close();
		}
	});

	test("History keyed removal works through a Fragment and a component wrapper", async () => {
		const tui = await AppHarness.open({ width: 24, height: 4 });
		const nativeHost = nativeHostForReact(tui) as
			| NativeTuiHostContract
			| undefined;
		if (nativeHost === undefined)
			throw new Error("native host association is unavailable");
		let lastCommitOpcodes: number[] = [];
		const nativeCommit = nativeHost.commitUiV1.bind(nativeHost);
		nativeHost.commitUiV1 = (words, ...args) => {
			lastCommitOpcodes = commitOpcodes(words);
			return nativeCommit(words, ...args);
		};
		const root = createReactRoot(tui);
		const WrappedUnit = (props: { readonly children?: ReactNode }): ReactNode =>
			createElement(HistoryUnit, {}, props.children);
		try {
			await root.render(
				createElement(
					History,
					{},
					createElement(
						Fragment,
						{ key: "fragment" },
						createElement(HistoryUnit, { key: "single" }, "single"),
					),
					createElement(WrappedUnit, { key: "wrapped" }, "wrapped"),
				),
			);
			await root.whenVisible();
			expect(tui.screenRows().join("\n")).toContain("single");
			expect(tui.screenRows().join("\n")).toContain("wrapped");

			await root.render(
				createElement(
					History,
					{},
					createElement(
						Fragment,
						{ key: "fragment" },
						createElement(HistoryUnit, { key: "single" }, "single"),
					),
				),
			);
			await root.whenVisible();
			expect(tui.screenRows().join("\n")).toContain("single");
			expect(tui.screenRows().join("\n")).not.toContain("wrapped");
			expect(lastCommitOpcodes).toContain(UI_OPCODES.historyAction);
			expect(lastCommitOpcodes).not.toContain(UI_OPCODES.retireRoot);
		} finally {
			root.close();
			tui.close();
		}
	});

	test("History removal rejects a non-tail or frozen unit", async () => {
		const nonTailTui = await AppHarness.open({ width: 24, height: 4 });
		const nonTailRoot = createReactRoot(nonTailTui);
		try {
			await nonTailRoot.render(
				createElement(
					History,
					{},
					createElement(HistoryUnit, { key: "first" }, "first"),
					createElement(HistoryUnit, { key: "tail" }, "tail"),
				),
			);
			await nonTailRoot.whenVisible();
			await expect(
				nonTailRoot.render(
					createElement(
						History,
						{},
						createElement(HistoryUnit, { key: "tail" }, "tail"),
					),
				),
			).rejects.toThrow("native UI commit rejected");
		} finally {
			nonTailRoot.close();
			nonTailTui.close();
		}

		const frozenTui = await AppHarness.open({ width: 24, height: 4 });
		const frozenRoot = createReactRoot(frozenTui);
		try {
			await frozenRoot.render(
				createElement(
					History,
					{},
					createElement(HistoryUnit, { action: "freeze" }, "frozen"),
				),
			);
			await frozenRoot.whenVisible();
			await expect(frozenRoot.render(null)).rejects.toThrow(
				"native UI commit rejected",
			);
		} finally {
			frozenRoot.close();
			frozenTui.close();
		}
	});

	test("nested History Source changes update the confirmed unit metrics", async () => {
		const tui = await AppHarness.open({ width: 24, height: 6 });
		const source = TextBlockSource.create();
		source.replace("one");
		const root = createReactRoot(tui);
		try {
			await root.render(
				createElement(
					History,
					{},
					createElement(HistoryUnit, {}, createElement(Content, { source })),
				),
			);
			await root.whenContentVisible();
			expect(tui.screenRows().some((row) => row.includes("one"))).toBe(true);
			source.replace("two longer");
			await root.whenContentVisible();
			expect(tui.screenRows().some((row) => row.includes("two longer"))).toBe(
				true,
			);
		} finally {
			root.close();
			tui.close();
			source.dispose();
		}
	});

	test("composite History metrics include every nested Content source", async () => {
		const tui = await AppHarness.open({ width: 24, height: 8 });
		const first = TextBlockSource.create();
		const second = TextBlockSource.create();
		first.replace("first");
		second.replace("second");
		const root = createReactRoot(tui);
		const render = (): ReactNode =>
			createElement(
				Fragment,
				{},
				createElement(
					History,
					{},
					createElement(HistoryUnit, { key: "following" }, "following"),
					createElement(
						HistoryUnit,
						{ key: "composite" },
						createElement(
							Column,
							{},
							createElement(Content, { source: first }),
							createElement(Content, { source: second }),
						),
					),
				),
				createElement(Box, {}, "body"),
			);
		try {
			await root.render(render());
			await root.whenContentVisible();
			const before = tui
				.screenRows()
				.findIndex((row) => row.includes("following"));
			expect(before).toBeGreaterThanOrEqual(0);
			second.replace("second\nthird\nfourth");
			await root.whenContentVisible();
			const after = tui
				.screenRows()
				.findIndex((row) => row.includes("following"));
			expect(after).toBeLessThan(before);
			expect(tui.screenRows().some((row) => row.includes("body"))).toBe(true);
		} finally {
			root.close();
			tui.close();
			first.dispose();
			second.dispose();
		}
	});

	test("History unit reorder is rejected by the native ownership contract", async () => {
		const tui = await AppHarness.open({ width: 24, height: 6 });
		const root = createReactRoot(tui);
		const renderOrder = (order: readonly string[]): ReactNode =>
			createElement(
				History,
				{},
				...order.map((label) =>
					createElement(HistoryUnit, { key: label }, label),
				),
			);
		try {
			await root.render(renderOrder(["a", "b"]));
			await root.whenVisible();
			await expect(root.render(renderOrder(["b", "a"]))).rejects.toThrow(
				"native UI commit rejected",
			);
		} finally {
			root.close();
			tui.close();
		}
	});

	test("a host has one React root authority", async () => {
		const tui = await AppHarness.open();
		const root = createReactRoot(tui);
		try {
			expect(() => createReactRoot(tui)).toThrow("already attached");
		} finally {
			root.close();
			tui.close();
		}
	});

	test("generated value forms pack RGB and attribute sentinels accepted by native", async () => {
		const tui = await AppHarness.open();
		const host = nativeHostForReact(tui) as NativeTuiHostContract | undefined;
		if (host === undefined)
			throw new Error("native host association is unavailable");
		let words: Uint32Array | undefined;
		const originalCommit = host.commitUiV1.bind(host);
		host.commitUiV1 = (nextWords, ...args) => {
			words = nextWords;
			return originalCommit(nextWords, ...args);
		};
		const root = createReactRoot(tui);
		try {
			await root.render(
				createElement(
					Box,
					{
						background: { type: "rgb", r: 1, g: 2, b: 3 },
						textAttributes: { bold: true },
					},
					"forms",
				),
			);
			expect(words).toBeDefined();
			expect(Array.from(words ?? [])).toContain(0x8000_0001);
		} finally {
			await root.unmount();
			tui.close();
		}
	});

	test("finite presentation values fail during pure React normalization", () => {
		expect(() =>
			Box({
				background: { type: "named", value: "not-an-ansi-color" } as never,
			} as never),
		).toThrow("unknown ANSI color");
		expect(() =>
			Box({
				background: { type: "indexed", value: 256 } as never,
			} as never),
		).toThrow("indexed ANSI color");
		expect(() =>
			Box({ style: { attributes: { unsupported: true } } } as never),
		).toThrow("unknown text attribute");
		expect(() =>
			Box({ style: StyleRef.theme("bad theme key") } as never),
		).toThrow("theme key must be non-empty");
		expect(() => Box({ styleStates: { severity: "" } } as never)).toThrow(
			"style state values must be nonempty NUL-free strings",
		);
		expect(() => Box({ styleStates: { "\0": "error" } } as never)).toThrow(
			"style state keys must be nonempty and NUL-free",
		);
		expect(() =>
			Text({ children: TextContent.markdown("**legacy**") } as never),
		).toThrow("text content must be a string, number, or bigint");
	});

	test("Row, Column, and Grid publish distinct generated layout properties", async () => {
		const tui = await AppHarness.open();
		const host = nativeHostForReact(tui) as NativeTuiHostContract | undefined;
		if (host === undefined)
			throw new Error("native host association is unavailable");
		let words: Uint32Array | undefined;
		const originalCommit = host.commitUiV1.bind(host);
		host.commitUiV1 = (nextWords, ...args) => {
			words = nextWords;
			return originalCommit(nextWords, ...args);
		};
		const root = createReactRoot(tui);
		const conveniences = [
			[Row, 1],
			[Column, 2],
			[Grid, 3],
		] as const;
		try {
			for (const [Convenience, expected] of conveniences) {
				await root.render(createElement(Convenience, {}, "layout"));
				assertLayoutProperty(words, expected);
			}
		} finally {
			await root.unmount();
			tui.close();
		}
	});

	test("portals use an owned typed root and reject cross-host targets", async () => {
		const first = await AppHarness.open();
		const second = await AppHarness.open();
		const firstHost = nativeHostForReact(first) as
			| NativeTuiHostContract
			| undefined;
		if (firstHost === undefined)
			throw new Error("native host association is unavailable");
		let words: Uint32Array | undefined;
		const originalCommit = firstHost.commitUiV1.bind(firstHost);
		firstHost.commitUiV1 = (nextWords, ...args) => {
			words = nextWords;
			return originalCommit(nextWords, ...args);
		};
		const firstRoot = createReactRoot(first);
		const secondRoot = createReactRoot(second);
		const portal = firstRoot.createPortal(createElement(Text, {}, "portal"));
		try {
			await firstRoot.render(createElement(Box, {}, portal));
			expect(Array.from(words ?? [])).toContain(UI_OPCODES.createRoot);
			await firstRoot.unmount();
			await expect(secondRoot.render(portal)).rejects.toThrow(
				"cross-host portal target",
			);
			expect(secondRoot.faulted).toBe(true);
		} finally {
			firstRoot.close();
			secondRoot.close();
			first.close();
			second.close();
		}
	});

	test("same-host portal roots reorder without entering the ordinary child list", async () => {
		const tui = await AppHarness.open();
		const root = createReactRoot(tui);
		const portalA = root.createPortal(createElement(Text, {}, "portal-a"), "a");
		const portalB = root.createPortal(createElement(Text, {}, "portal-b"), "b");
		const ordinary = (key: string, text: string) =>
			createElement(Text, { key }, text);
		try {
			await root.render([
				ordinary("before", "before"),
				portalA,
				ordinary("between", "between"),
				portalB,
				ordinary("after", "after"),
			]);
			await root.render([
				ordinary("after", "after"),
				portalB,
				ordinary("before", "before"),
				portalA,
				ordinary("between", "between"),
			]);
			await root.render([portalA]);
			await root.unmount();
			expect(root.faulted).toBe(false);
		} finally {
			root.close();
			tui.close();
		}
	});

	test("portal root owner transfer retires and recreates its typed root", async () => {
		const tui = await AppHarness.open();
		const host = nativeHostForReact(tui) as NativeTuiHostContract | undefined;
		if (host === undefined)
			throw new Error("native host association is unavailable");
		let transferWords: Uint32Array | undefined;
		const originalCommit = host.commitUiV1.bind(host);
		host.commitUiV1 = (words, ...args) => {
			transferWords = words;
			return originalCommit(words, ...args);
		};
		const root = createReactRoot(tui);
		const portal = root.createPortal(createElement(Text, {}, "owned"), "owned");
		try {
			await root.render([
				createElement(Box, { key: "left" }, portal),
				createElement(Box, { key: "right" }),
			]);
			await root.render([
				createElement(Box, { key: "left" }),
				createElement(Box, { key: "right" }, portal),
			]);
			expect(root.faulted).toBe(false);
			expect(commitOpcodes(transferWords ?? new Uint32Array())).toContain(
				UI_OPCODES.retireRoot,
			);
			expect(commitOpcodes(transferWords ?? new Uint32Array())).toContain(
				UI_OPCODES.createRoot,
			);
		} finally {
			root.close();
			tui.close();
		}
	});

	test("lazy content tokens materialize once, release on detach, and reject duplicate attachment", async () => {
		const tui = await AppHarness.open();
		const host = nativeHostForReact(tui) as NativeTuiHostContract | undefined;
		if (host === undefined)
			throw new Error("native host association is unavailable");
		const source = TextBlockSource.create();
		source.replace("token content");
		const createdCounts: number[] = [];
		const originalCommit = host.commitUiV1.bind(host);
		host.commitUiV1 = (...args) => {
			const acknowledgement = originalCommit(...args);
			createdCounts.push(acknowledgement[3] ?? 0);
			return acknowledgement;
		};
		function ContentConsumer() {
			const port = useContentPort();
			const connector = useContentConnector({ port, source });
			return createElement(Content, { port: connector });
		}
		const root = createReactRoot(tui);
		try {
			await root.render(
				createElement(Box, {}, createElement(ContentConsumer, {})),
			);
			expect(createdCounts).toEqual([4]);
			await root.render(
				createElement(Box, {}, createElement(ContentConsumer, {})),
			);
			expect(createdCounts).toEqual([4]);
			await root.unmount();
			expect(createdCounts).toEqual([4, 0, 0, 0]);
			await root.render(
				createElement(Box, {}, createElement(ContentConsumer, {})),
			);
			expect(createdCounts).toEqual([4, 0, 0, 0, 4]);

			function DuplicateConsumer() {
				const port = useContentPort();
				const connector = useContentConnector({ port, source });
				return createElement(
					Box,
					{},
					createElement(Content, { key: "one", port: connector }),
					createElement(Content, { key: "two", port: connector }),
				);
			}
			await expect(
				root.render(createElement(DuplicateConsumer, {})),
			).rejects.toThrow("native UI commit rejected");
			expect(root.faulted).toBe(true);
			root.close();
			const healthy = await AppHarness.open();
			const healthyRoot = createReactRoot(healthy);
			try {
				await healthyRoot.render(createElement(Box, {}, "healthy"));
				await healthyRoot.render(createElement(Box, {}, "healthy update"));
				await healthyRoot.unmount();
			} finally {
				healthyRoot.close();
				healthy.close();
			}
		} finally {
			tui.close();
			source.dispose();
		}
	});

	test("hook owner retains detached resources and releases them on owner cleanup", async () => {
		const tui = await AppHarness.open();
		const host = nativeHostForReact(tui) as NativeTuiHostContract | undefined;
		if (host === undefined)
			throw new Error("native host association is unavailable");
		const source = TextBlockSource.create();
		source.replace("owner lifetime");
		const commits: Array<{
			readonly created: number;
			readonly opcodes: readonly number[];
			readonly words: Uint32Array;
			readonly acknowledgement: Uint32Array;
		}> = [];
		const originalCommit = host.commitUiV1.bind(host);
		host.commitUiV1 = (words, ...args) => {
			const acknowledgement = originalCommit(words, ...args);
			commits.push({
				created: acknowledgement[3] ?? 0,
				opcodes: commitOpcodes(words),
				words,
				acknowledgement,
			});
			return acknowledgement;
		};
		function ComponentOwner({ show }: { readonly show: boolean }) {
			const port = useContentPort();
			const connector = useContentConnector({ port, source });
			return createElement(
				Box,
				{},
				show ? createElement(Content, { port: connector }) : null,
			);
		}
		const renderOwner = (show: boolean) =>
			createElement(StrictMode, {}, createElement(ComponentOwner, { show }));
		const root = createReactRoot(tui);
		try {
			await root.render(renderOwner(true));
			const first = commits[0];
			if (first === undefined)
				throw new Error("initial commit was not captured");
			expect(first.created).toBe(4);
			expect(first.opcodes).toContain(UI_OPCODES.createPort);
			expect(first.opcodes).toContain(UI_OPCODES.createConnector);
			const firstHandles = Array.from({ length: first.created }, (_, index) =>
				Array.from(first.acknowledgement.slice(8 + index * 4, 12 + index * 4)),
			);
			const firstPort = firstHandles.find((handle) => handle[3] === 2);
			const firstConnector = firstHandles.find((handle) => handle[3] === 3);
			if (firstPort === undefined || firstConnector === undefined)
				throw new Error("initial acknowledgement omitted token resources");

			await root.render(renderOwner(false));
			expect(commits[1]?.opcodes).not.toContain(UI_OPCODES.disposePort);
			expect(commits[1]?.opcodes).not.toContain(UI_OPCODES.disposeConnector);
			expect(() => source.dispose()).toThrow();

			await root.render(renderOwner(true));
			const restored = commits[2];
			expect(restored?.created).toBe(1);
			expect(restored?.opcodes).not.toContain(UI_OPCODES.createPort);
			expect(restored?.opcodes).not.toContain(UI_OPCODES.createConnector);
			const selection = commitRecords(
				restored?.words ?? new Uint32Array(),
			).find((record) => record.opcode === UI_OPCODES.selectConnector);
			expect(selection?.operands.slice(0, 4)).toEqual(firstPort);
			expect(selection?.operands.slice(4, 8)).toEqual(firstConnector);

			await root.unmount();
			await Promise.resolve();
			await Promise.resolve();
			const disposePorts = commits.filter((commit) =>
				commit.opcodes.includes(UI_OPCODES.disposePort),
			);
			const disposeConnectors = commits.filter((commit) =>
				commit.opcodes.includes(UI_OPCODES.disposeConnector),
			);
			expect(disposePorts).toHaveLength(1);
			expect(disposeConnectors).toHaveLength(1);
			expect(source.disposed).toBe(false);
			source.dispose();
		} finally {
			root.close();
			tui.close();
			if (!source.disposed) source.dispose();
		}
	});

	test("committed Connector dependency replacement releases an unused old binding", async () => {
		const tui = await AppHarness.open();
		const host = nativeHostForReact(tui) as NativeTuiHostContract | undefined;
		if (host === undefined)
			throw new Error("native host association is unavailable");
		const firstSource = TextBlockSource.create();
		const secondSource = TextBlockSource.create();
		firstSource.replace("first");
		secondSource.replace("second");
		const opcodes: number[][] = [];
		const originalCommit = host.commitUiV1.bind(host);
		host.commitUiV1 = (words, ...args) => {
			const acknowledgement = originalCommit(words, ...args);
			opcodes.push(commitOpcodes(words));
			return acknowledgement;
		};
		function OptionalContent({
			show,
			source,
		}: {
			readonly show: boolean;
			readonly source: TextBlockSource;
		}) {
			const port = useContentPort();
			const connector = useContentConnector({ port, source });
			return createElement(
				Box,
				{},
				show ? createElement(Content, { port: connector }) : null,
			);
		}
		const root = createReactRoot(tui);
		try {
			await root.render(
				createElement(OptionalContent, { show: true, source: firstSource }),
			);
			await root.render(
				createElement(OptionalContent, { show: false, source: secondSource }),
			);
			await Promise.resolve();
			await Promise.resolve();
			expect(
				opcodes.some((records) =>
					records.includes(UI_OPCODES.disposeConnector),
				),
			).toBe(true);
			// The new token never reached a Content consumer, while the old
			// binding was released after the accepted hook dependency change.
			firstSource.dispose();
			secondSource.dispose();
			await root.unmount();
		} finally {
			root.close();
			tui.close();
			if (!firstSource.disposed) firstSource.dispose();
			if (!secondSource.disposed) secondSource.dispose();
		}
	});

	test("superseded token cleanup follows stale consumer detachment", async () => {
		const tui = await AppHarness.open();
		const host = nativeHostForReact(tui) as NativeTuiHostContract | undefined;
		if (host === undefined)
			throw new Error("native host association is unavailable");
		const firstSource = TextBlockSource.create();
		const secondSource = TextBlockSource.create();
		firstSource.replace("first");
		secondSource.replace("second");
		const opcodes: number[][] = [];
		const originalCommit = host.commitUiV1.bind(host);
		host.commitUiV1 = (words, ...args) => {
			const acknowledgement = originalCommit(words, ...args);
			opcodes.push(commitOpcodes(words));
			return acknowledgement;
		};
		const Consumer = memo(
			({
				token,
			}: {
				readonly token: ContentConnectorToken;
				readonly version: number;
			}) => createElement(Content, { port: token }),
			(left, right) => left.version === right.version,
		);
		function Owner({
			source,
			version,
		}: {
			readonly source: TextBlockSource;
			readonly version: number;
		}) {
			const port = useContentPort();
			const connector = useContentConnector({ port, source });
			return createElement(
				Box,
				{},
				createElement(Consumer, { token: connector, version }),
			);
		}
		const root = createReactRoot(tui);
		try {
			await root.render(
				createElement(Owner, { source: firstSource, version: 0 }),
			);
			await root.render(
				createElement(Owner, { source: secondSource, version: 0 }),
			);
			expect(
				opcodes.some((records) =>
					records.includes(UI_OPCODES.disposeConnector),
				),
			).toBe(false);
			await root.render(
				createElement(Owner, { source: secondSource, version: 1 }),
			);
			await Promise.resolve();
			await Promise.resolve();
			expect(
				opcodes.some((records) =>
					records.includes(UI_OPCODES.disposeConnector),
				),
			).toBe(true);
			firstSource.dispose();
			expect(() => secondSource.dispose()).toThrow();
			await root.unmount();
			await Promise.resolve();
			await Promise.resolve();
			secondSource.dispose();
		} finally {
			root.close();
			tui.close();
			if (!firstSource.disposed) firstSource.dispose();
			if (!secondSource.disposed) secondSource.dispose();
		}
	});

	test("a live hook token transfers its qualified resources without duplication", async () => {
		const tui = await AppHarness.open();
		const host = nativeHostForReact(tui) as NativeTuiHostContract | undefined;
		if (host === undefined)
			throw new Error("native host association is unavailable");
		const source = TextBlockSource.create();
		source.replace("transfer");
		const createdCounts: number[] = [];
		const originalCommit = host.commitUiV1.bind(host);
		host.commitUiV1 = (...args) => {
			const acknowledgement = originalCommit(...args);
			createdCounts.push(acknowledgement[3] ?? 0);
			return acknowledgement;
		};
		function Transfer({ target }: { readonly target: string }) {
			const port = useContentPort();
			const connector = useContentConnector({ port, source });
			return createElement(
				Box,
				{},
				createElement(Content, { key: target, port: connector }),
			);
		}
		const root = createReactRoot(tui);
		try {
			await root.render(createElement(Transfer, { target: "first" }));
			await root.render(createElement(Transfer, { target: "second" }));
			expect(createdCounts).toEqual([4, 1]);
			await root.unmount();
			expect(createdCounts).toEqual([4, 1, 0, 0, 0]);
		} finally {
			root.close();
			tui.close();
			source.dispose();
		}
	});

	test("same Content identity updates its hook Connector binding atomically", async () => {
		const tui = await AppHarness.open();
		const host = nativeHostForReact(tui) as NativeTuiHostContract | undefined;
		if (host === undefined)
			throw new Error("native host association is unavailable");
		const firstSource = TextBlockSource.create();
		const secondSource = TextBlockSource.create();
		firstSource.replace("first");
		secondSource.replace("second");
		const commits: Array<{
			readonly created: number;
			readonly opcodes: readonly number[];
			readonly words: Uint32Array;
		}> = [];
		const originalCommit = host.commitUiV1.bind(host);
		host.commitUiV1 = (words, ...args) => {
			const acknowledgement = originalCommit(words, ...args);
			commits.push({
				created: acknowledgement[3] ?? 0,
				opcodes: commitOpcodes(words),
				words,
			});
			return acknowledgement;
		};
		function DynamicContent({ source }: { readonly source: TextBlockSource }) {
			const port = useContentPort();
			const connector = useContentConnector({ port, source });
			return createElement(
				Box,
				{},
				createElement(Content, { port: connector }),
			);
		}
		const root = createReactRoot(tui);
		try {
			await root.render(createElement(DynamicContent, { source: firstSource }));
			expect(commits[0]?.created).toBe(4);
			await root.render(
				createElement(DynamicContent, { source: secondSource }),
			);
			expect(commits[1]?.created).toBe(1);
			expect(commits[1]?.opcodes).toContain(UI_OPCODES.createConnector);
			// The new token is selected by the accepted Content update first.  Its
			// old token is retired by the committed hook-dependency cleanup, not by
			// inferring ownership from the consumer selection operation.
			expect(commits[1]?.opcodes).not.toContain(UI_OPCODES.disposeConnector);
			expect(commits[2]?.opcodes).toContain(UI_OPCODES.disposeConnector);
			expect(commits[1]?.opcodes).not.toContain(UI_OPCODES.createPort);
			firstSource.dispose();
			expect(() => secondSource.dispose()).toThrow();
			await root.unmount();
			await Promise.resolve();
			await Promise.resolve();
			const connectorDisposals = commits.filter((commit) =>
				commit.opcodes.includes(UI_OPCODES.disposeConnector),
			);
			expect(connectorDisposals).toHaveLength(2);
			expect(secondSource.disposed).toBe(false);
			secondSource.dispose();
		} finally {
			root.close();
			tui.close();
			if (!firstSource.disposed) firstSource.dispose();
			if (!secondSource.disposed) secondSource.dispose();
		}
	});

	test("one instance keeps two unconditional Connector hooks reusable across A/B/A", async () => {
		const tui = await AppHarness.open();
		const host = required(
			nativeHostForReact(tui) as NativeTuiHostContract | undefined,
			"native host association is unavailable",
		);
		const sourceA = TextBlockSource.create();
		const sourceB = TextBlockSource.create();
		sourceA.replace("A");
		sourceB.replace("B");
		const commits: Array<{
			readonly created: number;
			readonly opcodes: readonly number[];
			readonly words: Uint32Array;
			readonly acknowledgement: Uint32Array;
		}> = [];
		const originalCommit = host.commitUiV1.bind(host);
		host.commitUiV1 = (words, ...args) => {
			const acknowledgement = originalCommit(words, ...args);
			commits.push({
				created: acknowledgement[3] ?? 0,
				opcodes: commitOpcodes(words),
				words,
				acknowledgement,
			});
			return acknowledgement;
		};
		function Switching({ selection }: { readonly selection: "A" | "B" }) {
			const port = useContentPort();
			const connectorA = useContentConnector({ port, source: sourceA });
			const connectorB = useContentConnector({ port, source: sourceB });
			return createElement(Content, {
				port: selection === "A" ? connectorA : connectorB,
			});
		}
		const root = createReactRoot(tui);
		try {
			await root.render(createElement(Switching, { selection: "A" }));
			const first = required(commits[0], "initial commit was not captured");
			const firstHandles = Array.from({ length: first.created }, (_, index) =>
				acknowledgedHandle(first.acknowledgement, index + 1),
			);
			const firstPort = required(
				firstHandles.find((handle) => handle[3] === 2),
				"initial acknowledgement omitted a Port",
			);
			const firstConnector = required(
				firstHandles.find((handle) => handle[3] === 3),
				"initial acknowledgement omitted a Connector",
			);

			await root.render(createElement(Switching, { selection: "B" }));
			const second = required(commits[1], "B commit was not captured");
			expect(second.created).toBe(1);
			expect(second.opcodes).not.toContain(UI_OPCODES.disposeConnector);
			expect(second.opcodes).not.toContain(UI_OPCODES.createPort);
			const secondSelection = required(
				selectedConnector(second.words),
				"B selection was not encoded",
			);
			expect(secondSelection.port).toEqual(firstPort);

			await root.render(createElement(Switching, { selection: "A" }));
			const third = required(commits[2], "A restoration was not captured");
			expect(third.created).toBe(0);
			expect(third.opcodes).not.toContain(UI_OPCODES.disposeConnector);
			const thirdSelection = required(
				selectedConnector(third.words),
				"A restoration selection was not encoded",
			);
			expect(thirdSelection.port).toEqual(firstPort);
			expect(thirdSelection.connector).toEqual(firstConnector);
			expect(() => sourceA.dispose()).toThrow();
			expect(() => sourceB.dispose()).toThrow();
			await root.unmount();
			await Promise.resolve();
			await Promise.resolve();
			sourceA.dispose();
			sourceB.dispose();
		} finally {
			root.close();
			tui.close();
			if (!sourceA.disposed) sourceA.dispose();
			if (!sourceB.disposed) sourceB.dispose();
		}
	});

	test("independent live Connector owners retain A/B/A identities across selection", async () => {
		const tui = await AppHarness.open();
		const host = nativeHostForReact(tui) as NativeTuiHostContract | undefined;
		if (host === undefined)
			throw new Error("native host association is unavailable");
		const sourceA = TextBlockSource.create();
		const sourceB = TextBlockSource.create();
		sourceA.replace("A");
		sourceB.replace("B");
		const commits: Array<{
			readonly created: number;
			readonly opcodes: readonly number[];
			readonly words: Uint32Array;
		}> = [];
		const originalCommit = host.commitUiV1.bind(host);
		host.commitUiV1 = (words, ...args) => {
			const acknowledgement = originalCommit(words, ...args);
			commits.push({
				created: acknowledgement[3] ?? 0,
				opcodes: commitOpcodes(words),
				words,
			});
			return acknowledgement;
		};
		function Choice({
			port,
			source,
			active,
		}: {
			readonly port: ContentPortToken;
			readonly source: TextBlockSource;
			readonly active: boolean;
		}) {
			const connector = useContentConnector({ port, source });
			return active ? createElement(Content, { port: connector }) : null;
		}
		function Switching({
			selection,
			keepA = true,
		}: {
			readonly selection: "A" | "B";
			readonly keepA?: boolean;
		}) {
			const port = useContentPort();
			return createElement(
				Box,
				{},
				keepA
					? createElement(Choice, {
							port,
							source: sourceA,
							active: selection === "A",
						})
					: null,
				createElement(Choice, {
					port,
					source: sourceB,
					active: selection === "B",
				}),
			);
		}
		const root = createReactRoot(tui);
		try {
			await root.render(createElement(Switching, { selection: "A" }));
			await root.render(createElement(Switching, { selection: "B" }));
			await root.render(createElement(Switching, { selection: "A" }));
			expect(commits.map((commit) => commit.created)).toEqual([4, 2, 1]);
			expect(commits[1]?.opcodes).not.toContain(UI_OPCODES.disposeConnector);
			expect(commits[2]?.opcodes).not.toContain(UI_OPCODES.createConnector);
			expect(
				selectedConnector(commits[1]?.words ?? new Uint32Array()),
			).toBeDefined();
			expect(() => sourceA.dispose()).toThrow();
			expect(() => sourceB.dispose()).toThrow();
			await root.render(
				createElement(Switching, { selection: "B", keepA: false }),
			);
			await Promise.resolve();
			await Promise.resolve();
			const inactiveCleanup = commits.at(-1);
			expect(inactiveCleanup?.opcodes).toContain(UI_OPCODES.disposeConnector);
			expect(inactiveCleanup?.opcodes).not.toContain(
				UI_OPCODES.selectConnector,
			);
			sourceA.dispose();
			expect(() => sourceB.dispose()).toThrow();
			await root.unmount();
			await Promise.resolve();
			await Promise.resolve();
			sourceB.dispose();
		} finally {
			root.close();
			tui.close();
			if (!sourceA.disposed) sourceA.dispose();
			if (!sourceB.disposed) sourceB.dispose();
		}
	});

	test("blocked Port cleanup waits for its dependent Connector without render rescans", async () => {
		const tui = await AppHarness.open();
		const host = nativeHostForReact(tui) as NativeTuiHostContract | undefined;
		if (host === undefined)
			throw new Error("native host association is unavailable");
		const source = TextBlockSource.create();
		source.replace("dependency");
		let port: ContentPortToken | undefined;
		const commits: number[][] = [];
		const originalCommit = host.commitUiV1.bind(host);
		host.commitUiV1 = (words, ...args) => {
			const acknowledgement = originalCommit(words, ...args);
			commits.push(commitOpcodes(words));
			return acknowledgement;
		};
		function PortOwner() {
			port = useContentPort();
			return createElement(Box, {});
		}
		function ConnectorOwner({ show }: { readonly show: boolean }) {
			if (port === undefined)
				throw new Error("Port owner did not render first");
			const connector = useContentConnector({ port, source });
			return show ? createElement(Content, { port: connector }) : null;
		}
		function App({
			keepPort,
			mountConnector,
			showConnector,
		}: {
			readonly keepPort: boolean;
			readonly mountConnector: boolean;
			readonly showConnector: boolean;
		}) {
			return createElement(
				Box,
				{},
				keepPort ? createElement(PortOwner) : null,
				port === undefined || !mountConnector
					? null
					: createElement(ConnectorOwner, { show: showConnector }),
			);
		}
		const root = createReactRoot(tui);
		try {
			await root.render(
				createElement(App, {
					keepPort: true,
					mountConnector: true,
					showConnector: false,
				}),
			);
			await root.render(
				createElement(App, {
					keepPort: true,
					mountConnector: true,
					showConnector: true,
				}),
			);
			await root.render(
				createElement(App, {
					keepPort: false,
					mountConnector: true,
					showConnector: true,
				}),
			);
			await Promise.resolve();
			await Promise.resolve();
			const commitsAfterBlockedCleanup = commits.length;
			await root.render(
				createElement(App, {
					keepPort: false,
					mountConnector: true,
					showConnector: true,
				}),
			);
			await Promise.resolve();
			expect(commits.length).toBe(commitsAfterBlockedCleanup);

			await root.render(
				createElement(App, {
					keepPort: false,
					mountConnector: false,
					showConnector: false,
				}),
			);
			await Promise.resolve();
			await Promise.resolve();
			expect(
				commits.filter((records) =>
					records.includes(UI_OPCODES.disposeConnector),
				),
			).toHaveLength(1);
			expect(
				commits.filter((records) => records.includes(UI_OPCODES.disposePort)),
			).toHaveLength(1);
			source.dispose();
			await root.unmount();
		} finally {
			root.close();
			tui.close();
			if (!source.disposed) source.dispose();
		}
	});

	test("connector hook cleanup does not release a still-mounted parent Port", async () => {
		const tui = await AppHarness.open();
		const host = nativeHostForReact(tui) as NativeTuiHostContract | undefined;
		if (host === undefined)
			throw new Error("native host association is unavailable");
		const firstSource = TextBlockSource.create();
		const secondSource = TextBlockSource.create();
		firstSource.replace("child one");
		secondSource.replace("child two");
		const commits: Array<readonly number[]> = [];
		const originalCommit = host.commitUiV1.bind(host);
		host.commitUiV1 = (words, ...args) => {
			const acknowledgement = originalCommit(words, ...args);
			commits.push(commitOpcodes(words));
			return acknowledgement;
		};
		function Child({
			port,
			source,
		}: {
			readonly port: ContentPortToken;
			readonly source: TextBlockSource;
		}) {
			const connector = useContentConnector({ port, source });
			return createElement(Content, { port: connector });
		}
		function Parent({
			show,
			source,
		}: {
			readonly show: boolean;
			readonly source: TextBlockSource;
		}) {
			const port = useContentPort();
			return createElement(
				Box,
				{},
				show ? createElement(Child, { port, source }) : null,
			);
		}
		const root = createReactRoot(tui);
		try {
			await root.render(
				createElement(Parent, { show: true, source: firstSource }),
			);
			expect(commits[0]).toContain(UI_OPCODES.createPort);
			expect(commits[0]).toContain(UI_OPCODES.createConnector);
			await root.render(
				createElement(Parent, { show: false, source: firstSource }),
			);
			await Promise.resolve();
			await Promise.resolve();
			expect(
				commits.some((opcodes) =>
					opcodes.includes(UI_OPCODES.disposeConnector),
				),
			).toBe(true);
			firstSource.dispose();
			await root.render(
				createElement(Parent, { show: true, source: secondSource }),
			);
			expect(commits.at(-1)).toContain(UI_OPCODES.createConnector);
			expect(commits.at(-1)).not.toContain(UI_OPCODES.createPort);
			expect(() => secondSource.dispose()).toThrow();
			await root.unmount();
			await Promise.resolve();
			await Promise.resolve();
			const portDisposals = commits.filter((opcodes) =>
				opcodes.includes(UI_OPCODES.disposePort),
			);
			expect(portDisposals).toHaveLength(1);
			expect(secondSource.disposed).toBe(false);
			secondSource.dispose();
		} finally {
			root.close();
			tui.close();
			if (!firstSource.disposed) firstSource.dispose();
			if (!secondSource.disposed) secondSource.dispose();
		}
	});

	test("Content transitions preserve accepted bindings across source, literal, and lazy modes", async () => {
		const tui = await AppHarness.open();
		const host = nativeHostForReact(tui) as NativeTuiHostContract | undefined;
		if (host === undefined)
			throw new Error("native host association is unavailable");
		const firstSource = TextBlockSource.create();
		const secondSource = TextBlockSource.create();
		const thirdSource = TextBlockSource.create();
		firstSource.replace("first source mode");
		secondSource.replace("second source mode");
		thirdSource.replace("third source mode");
		const commits: Array<readonly number[]> = [];
		const originalCommit = host.commitUiV1.bind(host);
		host.commitUiV1 = (words, ...args) => {
			const acknowledgement = originalCommit(words, ...args);
			commits.push(commitOpcodes(words));
			return acknowledgement;
		};
		function Modes({
			mode,
			source,
		}: {
			readonly mode: "source" | "literal" | "lazy";
			readonly source: TextBlockSource;
		}) {
			const port = useContentPort();
			const connector = useContentConnector({ port, source });
			void connector;
			if (mode === "source") return createElement(Content, { source });
			if (mode === "literal") return createElement(Content, {}, "literal mode");
			return createElement(Content, { port });
		}
		const root = createReactRoot(tui);
		try {
			await root.render(
				createElement(Modes, { mode: "source", source: firstSource }),
			);
			await root.render(
				createElement(Modes, { mode: "literal", source: firstSource }),
			);
			// The implicit source Connector is retired while this occurrence is
			// still mounted; no occurrence unmount is required to release it.
			firstSource.dispose();
			await root.render(
				createElement(Modes, { mode: "source", source: secondSource }),
			);
			await root.render(
				createElement(Modes, { mode: "lazy", source: secondSource }),
			);
			// Replacing an implicit binding with the explicit Port retires the
			// old occurrence-owned Port and Connector while the occurrence lives.
			secondSource.dispose();
			await root.render(
				createElement(Modes, { mode: "source", source: thirdSource }),
			);
			expect(commits.length).toBe(5);
			expect(
				commits.some((opcodes) => opcodes.includes(UI_OPCODES.createPort)),
			).toBe(true);
			expect(
				commits.some((opcodes) => opcodes.includes(UI_OPCODES.replaceLiteral)),
			).toBe(true);
			expect(
				commits.some((opcodes) => opcodes.includes(UI_OPCODES.createConnector)),
			).toBe(true);
			await root.unmount();
		} finally {
			root.close();
			tui.close();
			if (!firstSource.disposed) firstSource.dispose();
			if (!secondSource.disposed) secondSource.dispose();
			if (!thirdSource.disposed) thirdSource.dispose();
		}
	});

	test("a committed token cannot cross React root authorities", async () => {
		const first = await AppHarness.open();
		const second = await AppHarness.open();
		const source = TextBlockSource.create();
		source.replace("authority");
		let shared: ContentConnectorToken | undefined;
		function Producer() {
			const port = useContentPort();
			const connector = useContentConnector({ port, source });
			shared = connector;
			return createElement(Content, { port: connector });
		}
		const firstRoot = createReactRoot(first);
		const secondRoot = createReactRoot(second);
		try {
			await firstRoot.render(createElement(Producer));
			if (shared === undefined) throw new Error("token was not captured");
			await expect(
				secondRoot.render(createElement(Content, { port: shared })),
			).rejects.toThrow("another React root");
			expect(secondRoot.faulted).toBe(true);
		} finally {
			firstRoot.close();
			secondRoot.close();
			first.close();
			second.close();
			if (!source.disposed) source.dispose();
		}
	});

	test("fabricated lazy tokens are rejected before native publication", async () => {
		const tui = await AppHarness.open();
		const host = nativeHostForReact(tui) as NativeTuiHostContract | undefined;
		if (host === undefined)
			throw new Error("native host association is unavailable");
		let nativeCalls = 0;
		const originalCommit = host.commitUiV1.bind(host);
		host.commitUiV1 = (...args) => {
			nativeCalls += 1;
			return originalCommit(...args);
		};
		const root = createReactRoot(tui);
		try {
			const fabricated = {
				kind: "content-port-token",
				family: "text",
			} as ContentPortToken;
			await expect(
				root.render(createElement(Content, { port: fabricated })),
			).rejects.toThrow("live qualified token");
			expect(nativeCalls).toBe(0);
			expect(root.faulted).toBe(true);
		} finally {
			root.close();
			tui.close();
		}
	});

	test("a HostConfig candidate can be abandoned without a native creation", async () => {
		const tui = await AppHarness.open();
		const root = createReactRoot(tui);
		const host = nativeHostForReact(tui) as NativeTuiHostContract | undefined;
		if (host === undefined)
			throw new Error("native host association is unavailable");
		const candidatesBefore = hostCandidateCreations();
		let nativeCalls = 0;
		const originalCommit = host.commitUiV1.bind(host);
		host.commitUiV1 = (...args) => {
			nativeCalls += 1;
			return originalCommit(...args);
		};
		function AbortAfterCandidate() {
			const candidate = createElement(Box, { key: "candidate" }, "candidate");
			function ThrowDuringRender(): never {
				throw new Error("abandoned render");
			}
			return [candidate, createElement(ThrowDuringRender, { key: "throw" })];
		}
		try {
			await expect(
				root.render(createElement(AbortAfterCandidate, {})),
			).rejects.toThrow("abandoned render");
			expect(hostCandidateCreations()).toBeGreaterThan(candidatesBefore);
			expect(nativeCalls).toBe(0);
			expect(root.faulted).toBe(true);
		} finally {
			root.close();
			tui.close();
		}
	});

	test("a yielded transition is abandoned by a synchronous replacement", async () => {
		const tui = await AppHarness.open({ width: 40, height: 8 });
		const host = required(
			nativeHostForReact(tui) as NativeTuiHostContract | undefined,
			"native host association is unavailable",
		);
		const source = TextBlockSource.create();
		source.replace("candidate content");
		let beginTransition: (() => void) | undefined;
		let nativeCalls = 0;
		const committedOpcodes: number[][] = [];
		let candidateEffectMounts = 0;
		let candidateRefCallbacks = 0;
		const originalCommit = host.commitUiV1.bind(host);
		host.commitUiV1 = (words, ...args) => {
			nativeCalls += 1;
			const opcodes = commitOpcodes(words);
			committedOpcodes.push(opcodes);
			return originalCommit(words, ...args);
		};

		function Candidate({ id }: { readonly id: number }) {
			const port = useContentPort();
			const connector = useContentConnector({ port, source });
			useLayoutEffect(() => {
				candidateEffectMounts += 1;
			}, []);
			return createElement(Content, {
				key: id,
				port: connector,
				ref: (_value: OccurrenceRef | null) => {
					candidateRefCallbacks += 1;
				},
			});
		}

		function App() {
			const [interrupted, setInterrupted] = useState(false);
			beginTransition = () =>
				startTransition(() => {
					setInterrupted(true);
				});
			const children = interrupted
				? Array.from({ length: 2_048 }, (_, id) =>
						createElement(Candidate, { key: id, id }),
					)
				: [createElement(Text, { key: "accepted" }, "accepted")];
			return createElement(Box, {}, children);
		}

		const root = createReactRoot(tui);
		try {
			await root.render(createElement(App));
			expect(nativeCalls).toBe(1);
			expect(candidateEffectMounts).toBe(0);
			expect(candidateRefCallbacks).toBe(0);
			const candidatesBefore = hostCandidateCreations();
			required(beginTransition, "transition trigger was not published")();

			// Let the installed Scheduler run the transition.  The condition is
			// candidate creation, not elapsed time: a changed count while the
			// native commit count is unchanged is direct evidence that the
			// concurrent render yielded before its mutation phase.
			await waitForCandidateYield(
				candidatesBefore,
				() => hostCandidateCreations(),
				() => nativeCalls,
				1,
			);
			expect(committedOpcodes).toHaveLength(1);

			// Acceptance supersedes the yielded transition without giving any
			// candidate Content token a native owner. Physical content delivery
			// remains asynchronous and has its own barrier below.
			await root.render(createElement(Text, {}, "replacement"));
			expect(nativeCalls).toBe(2);
			expect(committedOpcodes.at(-1)).toEqual(
				expect.arrayContaining([UI_OPCODES.replaceLiteral]),
			);
			await root.whenContentVisible();
			expect(tui.screenRows().some((row) => row.includes("replacement"))).toBe(
				true,
			);
			expect(candidateEffectMounts).toBe(0);
			expect(candidateRefCallbacks).toBe(0);
			expect(committedOpcodes.flat()).not.toContain(UI_OPCODES.createConnector);

			// Mount the stateful app again and unmount while a second transition is
			// still yielded. This checks that the public unmount Promise supersedes
			// pending work and that close can then finish ordinary cleanup.
			await root.render(createElement(App));
			expect(nativeCalls).toBe(3);
			const unmountCandidatesBefore = hostCandidateCreations();
			required(beginTransition, "transition trigger was not republished")();
			await waitForCandidateYield(
				unmountCandidatesBefore,
				() => hostCandidateCreations(),
				() => nativeCalls,
				3,
			);
			const unmounted = await root.unmount();
			expect(unmounted.accepted).toBe(true);
			expect(nativeCalls).toBe(4);
			expect(candidateEffectMounts).toBe(0);
			expect(candidateRefCallbacks).toBe(0);

			// A subsequent public render proves that the root was not faulted or
			// left with an abandoned ownership journal.
			await root.render(createElement(Text, {}, "healthy"));
			expect(nativeCalls).toBe(5);
			expect(root.faulted).toBe(false);
			await root.whenContentVisible();
			expect(tui.screenRows().some((row) => row.includes("healthy"))).toBe(
				true,
			);

			// Close itself must also supersede pending work, not only a public
			// unmount. Start one final yielded transition and close the root before
			// its mutation phase can materialize the lazy resources.
			await root.render(createElement(App));
			expect(nativeCalls).toBe(6);
			const closeCandidatesBefore = hostCandidateCreations();
			required(beginTransition, "transition trigger was not republished")();
			await waitForCandidateYield(
				closeCandidatesBefore,
				() => hostCandidateCreations(),
				() => nativeCalls,
				6,
			);
			root.close();
			await new Promise<void>((resolve) => setTimeout(resolve, 0));
			expect(nativeCalls).toBe(6);
			expect(candidateEffectMounts).toBe(0);
			expect(candidateRefCallbacks).toBe(0);
			expect(committedOpcodes.flat()).not.toContain(UI_OPCODES.createConnector);
		} finally {
			root.close();
			tui.close();
			source.dispose();
		}
	});

	test("fault cleanup reports failure and permits an explicit retry", async () => {
		const tui = await AppHarness.open({ width: 32, height: 8 });
		const host = nativeHostForReact(tui) as NativeTuiHostContract | undefined;
		if (host === undefined)
			throw new Error("native host association is unavailable");
		const root = createReactRoot(tui);
		try {
			await root.render(createElement(Box, {}, "accepted cleanup"));
			host.commitUiV1 = () =>
				new Uint32Array([0x8000_0000, 1, 0, 0, 0, 6, 0, 0]);
			await expect(
				root.render(createElement(Box, {}, "fault")),
			).rejects.toThrow("native UI commit rejected");
			const closeUiState = host.closeUiState.bind(host);
			let failOnce = true;
			host.closeUiState = () => {
				if (failOnce) {
					failOnce = false;
					throw new Error("injected cleanup failure");
				}
				closeUiState();
			};
			expect(() => root.close()).toThrow("injected cleanup failure");
			expect(root.faulted).toBe(true);
			await expect(
				root.render(createElement(Box, {}, "after close failure")),
			).rejects.toThrow("React root is closed");
			root.close();
		} finally {
			tui.close();
		}
	});
});
