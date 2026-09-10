import { native, requireNativeClass } from "../transport/native/addon.ts";
import { asTuiError, tuiError } from "../api/errors.ts";
import { materializeTheme, type Theme } from "../api/presentation/theme.ts";
import type { ContentPortOptions } from "../api/content/retained.ts";
import { TextContent } from "../api/content/text-content.ts";
import type { ContentPort } from "../api/content/explicit.ts";
import {
	markReactHostClosed,
	reactRootAuthority,
	registerReactHost,
} from "../react/host-registry.ts";
import type { NativeTuiHostContract } from "../transport/native/addon.ts";
import type { TuiEvent } from "./events.ts";
import type { RuntimeErrorReporter } from "./error-channel.ts";
import { setRuntimeErrorReporter } from "./diagnostics.ts";
import { OutputWaitOwner } from "./output-waiter.ts";

export interface TerminalMetadata {
	readonly width: number;
	readonly height: number;
}

export interface TuiOpenOptions {
	readonly width?: number;
	readonly height?: number;
	readonly headless?: boolean;
	readonly signal?: AbortSignal;
	readonly theme?: Theme;
}

export interface TuiRuntime {
	readonly size: TerminalMetadata;
	nextEvent(signal?: AbortSignal): Promise<TuiEvent>;
	onRuntimeError(listener: RuntimeErrorReporter): () => void;
	resize(width: number, height: number): void;
	close(): void;
	exit(): void;
	contentPort(options?: ContentPortOptions | typeof TextContent): ContentPort;
	bindKey(key: string, routeId: string, modifiers?: readonly string[]): void;
	forwardPaste(text: string): void;
	setTheme(theme: Theme): void;
}

export class Tui implements TuiRuntime {
	private closed = false;
	private readonly host: NativeTuiHostContract;
	private width: number;
	private height: number;
	private runtimeErrorListener: RuntimeErrorReporter | undefined;
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
		registerReactHost(this, host);
	}

	static async open(options: TuiOpenOptions = {}): Promise<Tui> {
		if (options.signal?.aborted)
			throw tuiError("cancelled", "TUI open was cancelled");
		const width = options.width ?? 80;
		const height = options.height ?? 24;
		validateSize(width, height);
		const Host = requireNativeClass(native.NativeTuiHost, "NativeTuiHost");
		let host: NativeTuiHostContract | undefined;
		try {
			host = new Host(width, height, options.headless ?? false);
			const tui = new Tui(host, width, height);
			if (options.theme !== undefined) tui.setTheme(options.theme);
			return tui;
		} catch (error) {
			try {
				host?.dispose();
			} catch (cleanupError) {
				throw new AggregateError(
					[asTuiError(error), asTuiError(cleanupError)],
					"TUI open cleanup failed",
				);
			}
			throw asTuiError(error);
		}
	}

	get size(): TerminalMetadata {
		return { width: this.width, height: this.height };
	}

	async nextEvent(signal?: AbortSignal): Promise<TuiEvent> {
		if (signal?.aborted)
			throw tuiError("cancelled", "TUI event wait was cancelled");
		if (this.closed) return { type: "terminate", reason: "closed" };
		const output = await this.outputWaiter.wait(signal);
		if (output === null) return { type: "terminate", reason: "closed" };
		return {
			type: "output",
			routeId: output.route_id,
			...(output.payload === undefined || output.payload === null
				? {}
				: { payload: output.payload }),
		};
	}

	onRuntimeError(listener: RuntimeErrorReporter): () => void {
		this.ensureOpen();
		if (typeof listener !== "function")
			throw new TypeError("runtime error listener must be a function");
		setRuntimeErrorReporter(this, listener);
		this.runtimeErrorListener = listener;
		return () => {
			if (this.runtimeErrorListener === listener) {
				this.runtimeErrorListener = undefined;
				setRuntimeErrorReporter(this, undefined);
			}
		};
	}

	contentPort(
		options: ContentPortOptions | typeof TextContent = {},
	): ContentPort {
		this.ensureOpen();
		const authority = reactRootAuthority(this);
		if (authority === undefined) {
			throw tuiError(
				"terminal",
				"TUI_CONTENT_PORT_REQUIRES_REACT_ROOT: createReactRoot(tui) must be called first",
			);
		}
		const family =
			options === TextContent
				? "text"
				: options !== null && typeof options === "object"
					? (options.family ?? "text")
					: undefined;
		if (
			options !== TextContent &&
			options !== null &&
			typeof options === "object"
		) {
			for (const key of Object.keys(options)) {
				if (key !== "family")
					throw tuiError(
						"validation",
						`unknown ContentPort option ${JSON.stringify(key)}`,
					);
			}
		}
		if (family !== "text")
			throw tuiError("validation", "unsupported ContentPort family");
		try {
			return authority.coordinator.createExplicitPort(family);
		} catch (error) {
			throw asTuiError(error);
		}
	}

	bindKey(key: string, routeId: string, modifiers?: readonly string[]): void {
		this.ensureOpen();
		try {
			this.host.bindKey(key, modifiers, routeId);
		} catch (error) {
			throw asTuiError(error);
		}
	}

	forwardPaste(text: string): void {
		this.ensureOpen();
		try {
			this.host.forwardPaste(text);
		} catch (error) {
			throw asTuiError(error);
		}
	}

	resize(width: number, height: number): void {
		this.ensureOpen();
		validateSize(width, height);
		try {
			this.host.resize(width, height);
			this.width = width;
			this.height = height;
		} catch (error) {
			throw asTuiError(error);
		}
	}

	setTheme(theme: Theme): void {
		this.ensureOpen();
		try {
			this.host.setTheme(materializeTheme(theme));
		} catch (error) {
			throw asTuiError(error);
		}
	}

	close(): void {
		if (this.closed) return;
		this.closed = true;
		this.outputWaiter.close();
		markReactHostClosed(this);
		setRuntimeErrorReporter(this, undefined);
		try {
			reactRootAuthority(this)?.close();
		} finally {
			this.host.dispose();
		}
	}

	exit(): void {
		if (this.closed) return;
		this.closed = true;
		this.outputWaiter.close();
		markReactHostClosed(this);
		setRuntimeErrorReporter(this, undefined);
		let failure: unknown;
		try {
			this.host.exit();
		} catch (error) {
			failure = asTuiError(error);
		}
		try {
			reactRootAuthority(this)?.closeAfterHostExit();
		} catch (error) {
			failure =
				failure === undefined
					? error
					: new AggregateError([failure, error], "TUI exit cleanup failed");
		}
		if (failure !== undefined) throw failure;
	}

	private ensureOpen(): void {
		if (this.closed) throw tuiError("terminal", "TUI runtime is closed");
	}
}

function validateSize(width: number, height: number): void {
	if (
		!Number.isInteger(width) ||
		!Number.isInteger(height) ||
		width <= 0 ||
		height <= 0 ||
		width > 65535 ||
		height > 65535
	) {
		throw asTuiError(
			new RangeError("terminal size must be an integer from 1 to 65535"),
		);
	}
}
