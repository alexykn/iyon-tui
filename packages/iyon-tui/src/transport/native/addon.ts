/**
 * Private native contract for the generic TUI package.
 *
 * Application/session bindings deliberately do not belong here. The addon is
 * the S6 `iyon-tui-native` artifact and this contract exposes only framework
 * handles, direct-occurrence UI commits, and generic terminal operations.
 */

import { tuiError } from "../../api/errors.ts";
import { resolveNativeArtifact } from "./artifact.ts";

export interface NativeWake {
	readonly schedule_environment_drain: boolean;
}

export interface NativeTextSourceContract {
	dispose(): void;
	sourceId(): number;
	sourceGeneration(): number;
	environmentSlot(): number;
	environmentGeneration(): number;
	contentGeneration(): string | number;
	snapshot(): object;
	stats(): object;
	family(): string;
}

export interface NativeContentConnectorContract {
	activate(): NativeWake;
	deactivate(): NativeWake;
	dispose(): NativeWake;
	status(): object;
}

export interface NativeContentPortContract {
	dispose(): void;
	portId(): number;
	portGeneration(): number;
	family(): string;
	deactivate(): NativeWake;
	connect(
		source: NativeTextSourceContract,
		kind: "plain" | "markdown" | "diff" | "ansi",
		wrap: "word" | "grapheme" | "noWrap",
		hyperlinks: boolean,
		smooth: boolean,
		tickIntervalMs: number,
		spring: number,
		minUnitsPerSecond: number,
		maxUnitsPerSecond: number,
	): NativeContentConnectorContract;
	mounted(): boolean;
}

export interface NativeHostEpochs {
	readonly host_id: string | number;
	readonly desired_structural_revision: string | number;
	readonly visible_structural_revision: string | number;
	readonly visible_frame_revision: string | number;
	readonly pending_epoch: string | number;
	readonly committed_epoch: string | number;
}

export interface NativeTuiHostContract {
	dispose(): void;
	exit(): void;
	contentPort(family?: string): NativeContentPortContract;
	setTheme(theme: object): void;
	exited(): boolean;
	bindKey(
		key: string,
		modifiers: readonly string[] | undefined,
		routeId: string,
	): void;
	dispatchKey(key: string, modifiers?: readonly string[]): void;
	dispatchPaste(text: string): void;
	forwardPaste(text: string): void;
	nextOutput(): { route_id: string; payload?: string | null } | null;
	waitForOutput(): Promise<{
		route_id: string;
		payload?: string | null;
	} | null>;
	screenRows(): string[];
	nativeHistoryRows(): string[];
	epochs(): NativeHostEpochs;
	uiNamespace(): number;
	/** Private React adapter seam; never expose occurrence internals publicly. */
	uiBodyHandle(): {
		readonly host_namespace: number;
		readonly slot: number;
		readonly generation: number;
		readonly kind: number;
	};
	uiHistoryUnitIdentity(handle: readonly number[]): number | string | null;
	/** Authoritative status of a qualified UI ContentPort/Connector resource. */
	uiPortMounted?(handle: readonly number[]): boolean;
	uiConnectorStatus?(handle: readonly number[]): object;
	interceptPasteUi?(handle: readonly number[], routeId: string): void;
	waitForUiPresentation(
		revision: number,
		contentVisible: boolean,
	): Promise<void>;
	/** Waits for one owned native event batch; null means the UI lane closed. */
	waitForUiEvents(): Promise<
		| readonly {
				readonly host_namespace: number;
				readonly slot: number;
				readonly generation: number;
				readonly kind: number;
				readonly mask: number;
				readonly text?: string | null;
				readonly cursor_bytes?: number | null;
				readonly key?: string | null;
				readonly revision?: number | string | null;
		  }[]
		| null
	>;
	waitForUiFailure(): Promise<{
		readonly phase: string;
		readonly code: string;
		readonly attempted_ui_revision: string;
		readonly attempted_work_epoch: string;
		readonly diagnostic: string;
		readonly retryable: boolean;
	} | null>;
	focusUi(handle: readonly number[]): void;
	uiVisibleGeometry(handle: readonly number[]): {
		readonly x: number;
		readonly y: number;
		readonly width: number;
		readonly height: number;
	} | null;
	drainUiEvents(): readonly {
		readonly host_namespace: number;
		readonly slot: number;
		readonly generation: number;
		readonly kind: number;
		readonly mask: number;
		readonly text?: string | null;
		readonly cursor_bytes?: number | null;
		readonly key?: string | null;
		readonly revision?: number | string | null;
	}[];
	setUiEventQueueLimits(maxRecords: number, maxBytes: number): void;
	failUiConnectorForTest(handle: readonly number[], diagnostic: string): void;
	closeUiState(): void;
	commitUiV1(
		words: Uint32Array,
		metadata: Uint8Array,
		ownedContent: Uint8Array,
		sources: readonly NativeTextSourceContract[],
	): Uint32Array;
	flushPendingHosts(
		budget?: number,
		forceRetry?: boolean,
	): {
		readonly rearm: boolean;
		readonly waiting_for_presentation: boolean;
		readonly attempted: number;
		readonly commits: readonly {
			readonly host_id: string | number;
			readonly committed_epoch: string | number;
			readonly visible_structural_revision: string | number;
		}[];
		readonly errors: readonly {
			readonly host_id: string | number;
			readonly attempted_epoch: string | number;
			readonly desired_revision: string | number;
			readonly phase: string;
			readonly code: string;
			readonly retryable: boolean;
			readonly diagnostic: string;
		}[];
		readonly wake_epoch: string | number;
	};
	resize(width: number, height: number): void;
	advanceTime(milliseconds: number): void;
	styleAt(row: number, column: number): object | null;
	cellXOfText(row: number, text: string): number | null;
}

export interface NativeTuiAddon {
	nativeVersion(): string;
	tuiSmoke(): string;
	NativeTuiHost?: new (
		width?: number,
		height?: number,
		headless?: boolean,
	) => NativeTuiHostContract;
	NativeTextSource?: new (
		kind?: "block" | "stream",
		options?: object,
	) => NativeTextSourceContract;
}

// The Node-API and direct loaders resolve this same canonical artifact path.
export const nativeArtifact = resolveNativeArtifact(import.meta.url);
const loadedNative = require(nativeArtifact.absolutePath) as NativeTuiAddon;
if (loadedNative.nativeVersion?.() !== nativeArtifact.packageBuildId) {
	throw new Error(
		`iyon-tui-native build identity mismatch at ${nativeArtifact.absolutePath}`,
	);
}
export const native = loadedNative;

/** Requires a native constructor without exposing addon details to callers. */
export function requireNativeClass<T>(factory: T | undefined, name: string): T {
	if (factory === undefined)
		throw tuiError("runtime", `${name} is unavailable in the native addon`);
	return factory;
}
