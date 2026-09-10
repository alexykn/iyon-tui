import { tuiError } from "../api/errors.ts";
import type { ContentPort } from "../api/content/explicit.ts";
import type { ContentPortOptions } from "../api/content/retained.ts";
import { TextContent } from "../api/content/text-content.ts";
import { materializeTheme, type Theme } from "../api/presentation/theme.ts";
import type { RuntimeErrorReporter } from "../runtime/error-channel.ts";
import { setRuntimeErrorReporter } from "../runtime/diagnostics.ts";
import type { TuiEvent } from "../runtime/events.ts";
import { OutputWaitOwner } from "../runtime/output-waiter.ts";
import type {
	TerminalMetadata,
	TuiOpenOptions,
	TuiRuntime,
} from "../runtime/runtime.ts";
import {
	markReactHostClosed,
	nativeHostForReact,
	reactRootAuthority,
	registerReactHost,
} from "../react/host-registry.ts";
import {
	native,
	requireNativeClass,
	type NativeTuiHostContract,
} from "../transport/native/addon.ts";

export interface AppHarnessContract extends TuiRuntime {
	pressKey(key: string, modifiers?: readonly string[]): void;
	paste(text: string): void;
	advance(ms: number): void;
	screenRows(): readonly string[];
	nativeHistoryRows(): readonly string[];
	styleAt(row: number, column: number): Readonly<Record<string, unknown>>;
	cellXOfText(row: number, text: string): number | null;
	exited(): boolean;
	now(): number;
	epochs(): AppHarnessEpochs;
}

export interface AppHarnessEpochs {
	readonly host_id: string | number;
	readonly desired_structural_revision: string | number;
	readonly visible_structural_revision: string | number;
	readonly visible_frame_revision: string | number;
	readonly pending_epoch: string | number;
	readonly committed_epoch: string | number;
}

export class AppHarness implements AppHarnessContract {
	private readonly host: NativeTuiHostContract;
	private width: number;
	private height: number;
	private clock = 0;
	private terminalExited = false;
	private readonly outputWaiter: OutputWaitOwner;

	private constructor(
		host: NativeTuiHostContract,
		width: number,
		height: number,
	) {
		this.host = host;
		this.width = width;
		this.height = height;
		this.outputWaiter = new OutputWaitOwner(() => this.host.waitForOutput());
	}

	static async open(options: TuiOpenOptions = {}): Promise<AppHarness> {
		if (options.signal?.aborted)
			throw tuiError("cancelled", "TUI open was cancelled");
		const Host = requireNativeClass(native.NativeTuiHost, "NativeTuiHost");
		const width = options.width ?? 80;
		const height = options.height ?? 24;
		const host = new Host(width, height, true);
		const harness = new AppHarness(host, width, height);
		registerReactHost(harness, host);
		if (options.theme !== undefined) harness.setTheme(options.theme);
		return harness;
	}

	get size(): TerminalMetadata {
		return { width: this.width, height: this.height };
	}

	contentPort(
		options: ContentPortOptions | typeof TextContent = {},
	): ContentPort {
		const authority = reactRootAuthority(this);
		if (authority === undefined)
			throw tuiError(
				"terminal",
				"TUI_CONTENT_PORT_REQUIRES_REACT_ROOT: createReactRoot(harness) must be called first",
			);
		const family =
			options === TextContent
				? "text"
				: ((options as ContentPortOptions).family ?? "text");
		return authority.coordinator.createExplicitPort(family);
	}

	async nextEvent(signal?: AbortSignal): Promise<TuiEvent> {
		if (signal?.aborted)
			throw tuiError("cancelled", "TUI event wait was cancelled");
		const output = await this.outputWaiter.wait(signal);
		if (output === null) return { type: "terminate", reason: "closed" };
		return {
			type: "output",
			routeId: output.route_id,
			...(output.payload == null ? {} : { payload: output.payload }),
		};
	}

	onRuntimeError(listener: RuntimeErrorReporter): () => void {
		setRuntimeErrorReporter(this, listener);
		return () => setRuntimeErrorReporter(this, undefined);
	}

	resize(width: number, height: number): void {
		this.host.resize(width, height);
		this.width = width;
		this.height = height;
	}
	bindKey(key: string, routeId: string, modifiers?: readonly string[]): void {
		this.host.bindKey(key, modifiers, routeId);
	}
	forwardPaste(text: string): void {
		this.host.forwardPaste(text);
	}
	setTheme(theme: Theme): void {
		this.host.setTheme(materializeTheme(theme));
	}

	pressKey(key: string, modifiers?: readonly string[]): void {
		this.host.dispatchKey(key, modifiers);
	}
	paste(text: string): void {
		this.host.dispatchPaste(text);
	}
	advance(ms: number): void {
		if (!Number.isSafeInteger(ms) || ms < 0)
			throw tuiError("validation", "clock advancement must be non-negative");
		this.host.advanceTime(ms);
		this.clock += ms;
	}
	screenRows(): readonly string[] {
		this.flush();
		return this.host.screenRows();
	}
	nativeHistoryRows(): readonly string[] {
		this.flush();
		return this.host.nativeHistoryRows();
	}
	styleAt(row: number, column: number): Readonly<Record<string, unknown>> {
		this.flush();
		return (this.host.styleAt(row, column) ?? {}) as Readonly<
			Record<string, unknown>
		>;
	}
	cellXOfText(row: number, text: string): number | null {
		this.flush();
		return this.host.cellXOfText(row, text);
	}
	exited(): boolean {
		return this.terminalExited || this.host.exited();
	}
	now(): number {
		return this.clock;
	}

	epochs(): AppHarnessEpochs {
		return this.host.epochs();
	}

	flush(): void {
		this.host.flushPendingHosts(1024, true);
	}

	close(): void {
		if (this.terminalExited) return;
		this.outputWaiter.close();
		markReactHostClosed(this);
		try {
			reactRootAuthority(this)?.close();
		} finally {
			setRuntimeErrorReporter(this, undefined);
			this.host.dispose();
		}
	}

	exit(): void {
		if (this.terminalExited) return;
		this.terminalExited = true;
		this.outputWaiter.close();
		markReactHostClosed(this);
		this.host.exit();
		reactRootAuthority(this)?.closeAfterHostExit();
		setRuntimeErrorReporter(this, undefined);
	}
}

export const createAppHarness = AppHarness.open;
