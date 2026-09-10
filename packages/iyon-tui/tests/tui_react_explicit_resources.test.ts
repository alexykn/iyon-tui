import { describe, expect, test } from "bun:test";
import { createElement } from "react";
import { TextFunnel, TextStreamSource, Tui } from "../src/index.ts";
import {
	Column,
	Content,
	createReactRoot,
	Editor,
	History,
	HistoryUnit,
} from "../src/react/index.ts";
import { nativeHostForReact } from "../src/react/host-registry.ts";
import { normalizeNativeFailure } from "../src/runtime/diagnostics.ts";
import type { NativeTuiHostContract } from "../src/transport/native/addon.ts";

describe("canonical React explicit content resources", () => {
	test("reserves one root authority before starting native consumers", async () => {
		const tui = await Tui.open({ headless: true, width: 24, height: 6 });
		const root = createReactRoot(tui);
		const host = (
			await import("../src/react/host-registry.ts")
		).nativeHostForReact(tui) as { waitForUiEvents(): Promise<unknown> };
		let duplicateWaiters = 0;
		const waitForUiEvents = host.waitForUiEvents.bind(host);
		host.waitForUiEvents = async () => {
			duplicateWaiters += 1;
			return waitForUiEvents();
		};
		try {
			expect(() => createReactRoot(tui)).toThrow("already attached");
			expect(duplicateWaiters).toBe(0);
			root.close();
			tui.close();
			expect(() => createReactRoot(tui)).toThrow("live TuiRuntime");
		} finally {
			try {
				root.close();
			} catch {}
			try {
				tui.close();
			} catch {}
		}
	});

	test("canceled output waits do not steal the next routed output", async () => {
		const tui = await Tui.open({ headless: true });
		const host = nativeHostForReact(tui) as NativeTuiHostContract;
		const drained = Promise.withResolvers<void>();
		const nativeWait = host.waitForOutput.bind(host);
		host.waitForOutput = async () => {
			const output = await nativeWait();
			drained.resolve();
			return output;
		};
		try {
			const controller = new AbortController();
			const canceled = tui.nextEvent(controller.signal);
			controller.abort();
			await expect(canceled).rejects.toThrow("cancelled");
			tui.bindKey("x", "after-cancel");
			host.dispatchKey("x");
			await drained.promise;
			await Promise.resolve();
			const next = await tui.nextEvent();
			expect(next).toMatchObject({ type: "output", routeId: "after-cancel" });
		} finally {
			tui.close();
		}
	});

	test("concurrent output waits consume distinct FIFO outputs", async () => {
		const tui = await Tui.open({ headless: true });
		const host = nativeHostForReact(tui) as {
			dispatchKey(key: string): void;
		};
		try {
			tui.bindKey("a", "first");
			tui.bindKey("b", "second");
			const first = tui.nextEvent();
			const second = tui.nextEvent();
			host.dispatchKey("a");
			host.dispatchKey("b");
			expect(await first).toMatchObject({ type: "output", routeId: "first" });
			expect(await second).toMatchObject({
				type: "output",
				routeId: "second",
			});
		} finally {
			tui.close();
		}
	});

	test("close wakes output waiters and remains idempotent", async () => {
		const tui = await Tui.open({ headless: true });
		const waiting = tui.nextEvent();
		tui.close();
		expect(await waiting).toEqual({ type: "terminate", reason: "closed" });
		tui.close();
		expect(await tui.nextEvent()).toEqual({
			type: "terminate",
			reason: "closed",
		});
	});

	test("reports native projection failure automatically without a presentation barrier", async () => {
		const tui = await Tui.open({ headless: true, width: 24, height: 6 });
		const root = createReactRoot(tui);
		const host = (
			await import("../src/react/host-registry.ts")
		).nativeHostForReact(tui) as {
			failUiConnectorForTest(
				handle: readonly number[],
				diagnostic: string,
			): void;
		};
		const source = TextStreamSource.create();
		const port = tui.contentPort();
		const connector = port.connect(source, TextFunnel.plain());
		const delivered = Promise.withResolvers<string>();
		const unsubscribe = tui.onRuntimeError((record) => {
			delivered.resolve(record.diagnostic);
			return true;
		});
		try {
			host.failUiConnectorForTest([0, 0, 0, 0], "automatic diagnostic probe");
			await root.render(createElement(Content, { port: connector }));
			expect(await delivered.promise).toContain("automatic diagnostic probe");
			expect(connector.status().phase).toBe("failed");
		} finally {
			unsubscribe();
			root.close();
			tui.close();
			source.dispose();
		}
	});
	test("native diagnostic ingress requires exact unsigned decimal revisions", () => {
		const failure = {
			phase: "content",
			code: "PROJECTION_FAILED",
			diagnostic: "projection failed",
			retryable: true,
			attempted_ui_revision: "18446744073709551615",
			attempted_work_epoch: "1",
		};
		expect(normalizeNativeFailure("host", failure)?.desiredRevision).toBe(
			0xffff_ffff_ffff_ffffn,
		);
		for (const invalid of ["18446744073709551616", -1n, 9007199254740992]) {
			expect(
				normalizeNativeFailure("host", {
					...failure,
					attempted_ui_revision: invalid,
				}),
			).toBeUndefined();
		}
	});
	test("requires a root, preserves caller resources across unmount, and reattaches", async () => {
		const tui = await Tui.open({ headless: true, width: 24, height: 6 });
		const source = TextStreamSource.create();
		source.append("explicit");
		try {
			expect(() => tui.contentPort()).toThrow(
				"TUI_CONTENT_PORT_REQUIRES_REACT_ROOT",
			);
			const root = createReactRoot(tui);
			const port = tui.contentPort();
			const connector = port.connect(source, TextFunnel.plain());
			expect(connector.status()).toMatchObject({
				phase: "idle",
				requested: false,
				visible: false,
			});

			await root.render(createElement(Content, { port: connector }));
			await root.whenContentVisible();
			expect(connector.status()).toMatchObject({
				phase: "active",
				requested: true,
				visible: true,
			});
			await root.unmount();
			await root.whenVisible();
			expect(port.mounted()).toBe(false);
			expect(connector.disposed).toBe(false);

			await root.render(createElement(Content, { port: connector }));
			await root.whenContentVisible();
			expect(connector.status().visible).toBe(true);

			await root.unmount();
			connector.deactivate();
			connector.dispose();
			port.dispose();
			await root.whenVisible();
			root.close();
		} finally {
			tui.close();
			source.dispose();
		}
	});

	test("rejects in-use disposal without faulting the React root", async () => {
		const tui = await Tui.open({ headless: true, width: 24, height: 6 });
		const root = createReactRoot(tui);
		const port = tui.contentPort();
		const source = TextStreamSource.create();
		const connector = port.connect(source, TextFunnel.plain());
		try {
			await root.render(createElement(Content, { port: connector }));
			await root.whenContentVisible();
			expect(() => connector.dispose()).toThrow();
			expect(root.faulted).toBe(false);
			await root.unmount();
			connector.deactivate();
			connector.dispose();
			port.dispose();
		} finally {
			root.close();
			source.dispose();
			tui.close();
		}
	});

	test("keeps imperative A/B selection coherent before and after React attachment", async () => {
		const tui = await Tui.open({ headless: true, width: 24, height: 6 });
		const root = createReactRoot(tui);
		const port = tui.contentPort();
		const sourceA = TextStreamSource.create();
		const sourceB = TextStreamSource.create();
		const connectorA = port.connect(sourceA, TextFunnel.plain());
		const connectorB = port.connect(sourceB, TextFunnel.plain());
		try {
			connectorA.activate();
			port.activate();
			expect(connectorA.status().requested).toBe(true);
			connectorB.activate();
			port.activate();
			expect(connectorB.status().requested).toBe(true);
			connectorA.deactivate();
			expect(connectorB.status().requested).toBe(true);
			await root.render(createElement(Content, { port }));
			await root.whenContentVisible();
			await root.render(
				createElement(Content, { port: connectorA, width: "fit" }),
			);
			connectorB.activate();
			await root.render(
				createElement(Content, { port: connectorA, width: "fill" }),
			);
			port.activate();
			expect(connectorB.status().requested).toBe(true);
			expect(connectorA.status().requested).toBe(false);
			await root.render(createElement(Content, { port }));
			port.activate();
			expect(connectorB.status().requested).toBe(true);
			connectorA.activate();
			port.activate();
			expect(connectorA.status().requested).toBe(true);
			await root.unmount();
			connectorA.deactivate();
			connectorA.dispose();
			connectorB.dispose();
			port.dispose();
		} finally {
			try {
				root.close();
			} catch {}
			try {
				tui.close();
			} catch {}
			sourceA.dispose();
			sourceB.dispose();
		}
	});

	test("clears the previous confirmed product on a successful switch and unmount", async () => {
		const tui = await Tui.open({ headless: true, width: 24, height: 6 });
		const root = createReactRoot(tui);
		const port = tui.contentPort();
		const sourceA = TextStreamSource.create();
		const sourceB = TextStreamSource.create();
		const connectorA = port.connect(sourceA, TextFunnel.plain());
		const connectorB = port.connect(sourceB, TextFunnel.plain());
		try {
			sourceA.append("A");
			sourceB.append("B");
			await root.render(createElement(Content, { port: connectorA }));
			await root.whenContentVisible();
			await root.render(createElement(Content, { port: connectorB }));
			await root.whenContentVisible();
			expect(connectorA.status()).toMatchObject({
				requested: false,
				visible: false,
			});
			expect(connectorB.status()).toMatchObject({
				requested: true,
				visible: true,
			});
			await root.unmount();
			await root.whenVisible();
			expect(connectorB.status().visible).toBe(false);
		} finally {
			connectorB.deactivate();
			connectorA.dispose();
			connectorB.dispose();
			port.dispose();
			root.close();
			tui.close();
			sourceA.dispose();
			sourceB.dispose();
		}
	});

	test("keeps the confirmed Connector visible when a requested switch fails", async () => {
		const tui = await Tui.open({ headless: true, width: 24, height: 6 });
		const root = createReactRoot(tui);
		const host = nativeHostForReact(tui) as {
			failUiConnectorForTest(
				handle: readonly number[],
				diagnostic: string,
			): void;
		};
		const port = tui.contentPort();
		const sourceA = TextStreamSource.create();
		const sourceB = TextStreamSource.create();
		const connectorA = port.connect(sourceA, TextFunnel.plain());
		const connectorB = port.connect(sourceB, TextFunnel.plain());
		try {
			sourceA.append("A");
			sourceB.append("B");
			await root.render(createElement(Content, { port: connectorA }));
			await root.whenContentVisible();
			host.failUiConnectorForTest([0, 0, 0, 0], "switch failed");
			await root.render(createElement(Content, { port: connectorB }));
			await expect(root.whenContentVisible()).rejects.toThrow("switch failed");
			expect(connectorA.status()).toMatchObject({
				requested: false,
				visible: true,
			});
			expect(connectorB.status()).toMatchObject({
				requested: true,
				visible: false,
				phase: "failed",
			});
		} finally {
			await root.unmount();
			connectorB.deactivate();
			connectorA.dispose();
			connectorB.dispose();
			port.dispose();
			root.close();
			sourceA.dispose();
			sourceB.dispose();
			tui.close();
		}
	});

	test("preserves Unicode Source retention and native smooth delivery", async () => {
		const tui = await Tui.open({ headless: true, width: 24, height: 6 });
		const root = createReactRoot(tui);
		const source = TextStreamSource.create({
			retention: { maxBytes: 8, overflow: "drop-oldest" },
		});
		const port = tui.contentPort();
		const connector = port.connect(source, TextFunnel.plain().smooth());
		const screenRows = () =>
			(nativeHostForReact(tui) as { screenRows(): string[] }).screenRows();
		try {
			source.append("αβγδε\n");
			source.append("終");
			const snapshot = source.snapshot();
			expect(snapshot.text).not.toContain("�");
			expect(source.stats().droppedHeadBytes).toBeGreaterThan(0n);
			await root.render(createElement(Content, { port: connector }));
			await root.whenContentVisible();
			expect(screenRows().join("\n")).toContain("終");
		} finally {
			await root.unmount();
			connector.deactivate();
			connector.dispose();
			port.dispose();
			root.close();
			tui.close();
			source.dispose();
		}
	});

	test("exports a physical History transfer witness and global input ordering", async () => {
		const tui = await Tui.open({ headless: true, width: 30, height: 6 });
		const root = createReactRoot(tui);
		const host = (
			await import("../src/react/host-registry.ts")
		).nativeHostForReact(tui) as {
			dispatchKey(key: string): void;
			dispatchPaste(text: string): void;
			nativeHistoryRows(): readonly string[];
			exited(): boolean;
		};
		const editorRef = {
			current: null as {
				focus(): void;
				interceptPaste(routeId: string): void;
			} | null,
		};
		const submission = Promise.withResolvers<string>();
		try {
			await root.render(
				createElement(
					Column,
					{},
					createElement(Editor, {
						ref: (value) => {
							editorRef.current = value as typeof editorRef.current;
						},
						defaultValue: "edit",
						onSubmit: (event) => submission.resolve(event.text),
					}),
				),
			);
			await root.whenVisible();
			expect(editorRef.current).not.toBeNull();
			const editor = editorRef.current!;
			editor.focus();
			tui.bindKey("x", "global");
			const globalEvent = tui.nextEvent();
			host.dispatchKey("x");
			expect(await globalEvent).toMatchObject({
				type: "output",
				routeId: "global",
			});
			editor.interceptPaste("paste-route");
			const pasteEvent = tui.nextEvent();
			host.dispatchPaste("pasted");
			expect(await pasteEvent).toMatchObject({
				type: "output",
				routeId: "paste-route",
				payload: "pasted",
			});
			tui.forwardPaste("forwarded");
			host.dispatchKey("Enter");
			expect(await submission.promise).toBe("editforwarded");
			await root.unmount();
			expect(editorRef.current).toBeNull();
			expect(() => editor.interceptPaste("retired-route")).toThrow();
			await root.render(
				createElement(
					History,
					{},
					createElement(HistoryUnit, {}, "history transfer"),
				),
			);
			await root.whenVisible();
			tui.exit();
			expect(host.nativeHistoryRows()).toContain("history transfer");
		} finally {
			root.close();
			tui.close();
		}
	});
});
