import { describe, expect, test } from "bun:test";
import {
	createElement,
	memo,
	StrictMode,
	useEffect,
	useLayoutEffect,
	useState,
} from "react";
import { StyleRef, TextBlockSource, TextContent } from "../src/index.ts";
import { hostCandidateCreations } from "../src/react/host-config.ts";
import { nativeHostForReact } from "../src/react/host-registry.ts";
import {
	Box,
	Column,
	Content,
	type ContentConnectorToken,
	type ContentPortToken,
	createReactRoot,
	Grid,
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

	test("acceptance and presentation barriers are distinct in T3", async () => {
		const tui = await AppHarness.open();
		const root = createReactRoot(tui);
		try {
			const result = await root.render(createElement(Text, {}, "accepted"));
			expect(root.whenVisible(result.revision)).rejects.toMatchObject({
				code: "T3_FRAME_BARRIER_UNREALIZED",
			});
		} finally {
			await root.unmount();
			tui.close();
		}
	});

	test("public refs publish overrides and reject unrealized focus/geometry", async () => {
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
					focus(): never;
					visibleGeometry(): Promise<never>;
			  }
			| undefined;
		try {
			await root.render(
				createElement(Box, {
					ref: (value) => {
						if (value !== null) ref = value as NonNullable<typeof ref>;
					},
				}),
			);
			if (ref === undefined) throw new Error("ref was not published");
			const instance = ref;
			expect(
				instance.setOverride("background", { type: "named", value: "red" })
					.accepted,
			).toBe(true);
			expect(instance.clearOverride("background").accepted).toBe(true);
			expect(() => instance.focus()).toThrow("T4 interaction executor");
			expect(instance.visibleGeometry()).rejects.toMatchObject({
				code: "T3_GEOMETRY_UNREALIZED",
			});
		} finally {
			await root.unmount();
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
			root.close();
		} finally {
			tui.close();
		}
	});
});
