import { createElement, type ReactNode } from "react";
import Reconciler from "react-reconciler";
import type { TuiRuntime } from "../runtime/runtime.ts";
import type { NativeTuiHostContract } from "../transport/native/addon.ts";
import {
	type CommitCoordinator,
	type NativeUiEvent,
	RootContainer,
} from "./commit.ts";
import { hostConfig } from "./host-config.ts";
import { nativeHostForReact } from "./host-registry.ts";

const activeRoots = new WeakSet<object>();
const closedHosts = new WeakSet<object>();
type RootLifecycle = "open" | "closing" | "closed";

export interface ReactCommit {
	readonly revision: number;
	readonly accepted: true;
}

export class ReactFrameBarrierError extends Error {
	readonly code = "FRAME_BARRIER_UNAVAILABLE" as const;

	constructor(
		message = "The requested native presentation barrier could not be completed",
	) {
		super(message);
		this.name = "ReactFrameBarrierError";
	}
}

export interface IyonReactRoot {
	render(children: ReactNode): Promise<ReactCommit>;
	createPortal(children: ReactNode, key?: string): ReactNode;
	whenVisible(revision?: number): Promise<void>;
	whenContentVisible(revision?: number): Promise<void>;
	unmount(): Promise<ReactCommit>;
	close(): void;
	readonly faulted: boolean;
}

export function createReactRoot(host: TuiRuntime): IyonReactRoot {
	const nativeHost = nativeHostForReact(host) as
		| NativeTuiHostContract
		| undefined;
	if (nativeHost === undefined)
		throw new TypeError("createReactRoot requires a live TuiRuntime host");
	if (activeRoots.has(nativeHost) || closedHosts.has(nativeHost))
		throw new Error("a React root is already attached to this Tui host");
	const root = new IyonRoot(nativeHost);
	activeRoots.add(nativeHost);
	return root;
}

class IyonRoot implements IyonReactRoot {
	private readonly host: NativeTuiHostContract;
	private readonly reconciler: ReturnType<typeof Reconciler>;
	private readonly container: RootContainer;
	private readonly coordinator: CommitCoordinator;
	private readonly opaqueRoot: ReturnType<
		typeof this.reconciler.createContainer
	>;
	private lifecycle: RootLifecycle = "open";
	private reactCleanupAttempted = false;
	private pending = new Set<{
		resolve(value: ReactCommit): void;
		reject(error: unknown): void;
	}>();

	constructor(host: NativeTuiHostContract) {
		this.host = host;
		this.reconciler = Reconciler(hostConfig);
		this.container = new RootContainer(host);
		this.coordinator = this.container.coordinator;
		this.opaqueRoot = this.reconciler.createContainer(
			this.container,
			0,
			null,
			false,
			null,
			"",
			(error) => {
				this.coordinator.faultFromReact(error);
				this.rejectPending(error);
			},
			(error) => this.rejectPending(error),
			(error) => this.rejectPending(error),
			() => {},
		);
		this.startNativeEventLane();
	}

	get faulted(): boolean {
		return this.coordinator.isFaulted;
	}

	render(children: ReactNode): Promise<ReactCommit> {
		if (this.lifecycle !== "open")
			return Promise.reject(new Error("React root is closed"));
		if (this.coordinator.isFaulted)
			return Promise.reject(this.coordinator.faultError);
		return new Promise<ReactCommit>((resolve, reject) => {
			const pending = { resolve, reject };
			this.pending.add(pending);
			try {
				this.coordinator.drainReleasedHookOwners();
				if (this.coordinator.isFaulted) {
					this.pending.delete(pending);
					reject(this.coordinator.faultError);
					return;
				}
				this.reconciler.updateContainerSync(
					children,
					this.opaqueRoot,
					null,
					() => {
						queueMicrotask(() => {
							this.coordinator.drainReleasedHookOwners();
							if (!this.pending.delete(pending)) return;
							if (this.coordinator.isFaulted)
								reject(this.coordinator.faultError);
							else
								resolve({
									revision: this.coordinator.acceptedUiRevision,
									accepted: true,
								});
						});
					},
				);
				this.reconciler.flushSyncFromReconciler();
			} catch (error) {
				this.pending.delete(pending);
				reject(error);
			}
		});
	}

	createPortal(children: ReactNode, key?: string): ReactNode {
		if (this.lifecycle !== "open") throw new Error("React root is closed");
		return createElement(
			"portal",
			{ key, portalOwner: this.container },
			children,
		);
	}

	whenVisible(revision?: number): Promise<void> {
		return this.awaitPresentation(revision, false);
	}

	whenContentVisible(revision?: number): Promise<void> {
		return this.awaitPresentation(revision, true);
	}

	async unmount(): Promise<ReactCommit> {
		return this.render(null);
	}

	close(): void {
		if (this.lifecycle === "closed") return;
		// Stop delivery before any React/native teardown.  The native wait is
		// awakened by closeUiState, and its already-owned batch is discarded by
		// the lane guard rather than calling a callback after close begins.
		this.lifecycle = "closing";
		let failure: unknown;
		if (!this.reactCleanupAttempted) {
			this.reactCleanupAttempted = true;
			try {
				this.coordinator.beginCleanup();
				this.reconciler.updateContainerSync(null, this.opaqueRoot, null, null);
				this.reconciler.flushSyncFromReconciler();
				this.reconciler.flushSyncWork();
			} catch (error) {
				failure = error;
				this.coordinator.abortCleanup();
			}
		}
		try {
			this.host.closeUiState();
		} catch (cleanupError) {
			failure =
				failure === undefined
					? cleanupError
					: new AggregateError(
							[failure, cleanupError],
							"React root cleanup failed",
						);
		}
		if (failure === undefined) {
			this.coordinator.finalizeCleanup();
			this.lifecycle = "closed";
			activeRoots.delete(this.host);
			closedHosts.add(this.host);
			this.rejectPending(new Error("React root was closed"));
		}
		if (failure !== undefined) throw failure;
	}

	private rejectPending(error: unknown): void {
		for (const pending of this.pending) pending.reject(error);
		this.pending.clear();
	}

	private async awaitPresentation(
		revision: number | undefined,
		contentVisible: boolean,
	): Promise<void> {
		if (this.lifecycle !== "open")
			return Promise.reject(new Error("React root is closed"));
		const target = revision ?? this.coordinator.acceptedUiRevision;
		try {
			// Native owns queue service, receipt waiting, deadlines, and exact
			// barrier completion. This call does not require a JS frame/tick pump.
			await this.host.waitForUiPresentation(target, contentVisible);
			return;
		} catch (error) {
			if (error instanceof ReactFrameBarrierError) throw error;
			throw new ReactFrameBarrierError(
				error instanceof Error ? error.message : String(error),
			);
		}
	}

	/**
	 * One root-owned asynchronous native event consumer.  It is deliberately
	 * separate from presentation barriers: native animation, input, and
	 * content work can produce events without a JS frame pump, while this
	 * consumer remains the sole owner that takes batches from the native queue.
	 */
	private startNativeEventLane(): void {
		void this.consumeNativeEventLane();
	}

	private async consumeNativeEventLane(): Promise<void> {
		while (this.lifecycle === "open") {
			let batch: readonly NativeUiEvent[] | null;
			try {
				batch = await this.host.waitForUiEvents();
			} catch (error) {
				if (this.lifecycle === "open") reportNativeEventError(error);
				return;
			}
			if (this.lifecycle !== "open" || batch === null) return;
			if (batch.length === 0) continue;
			try {
				this.coordinator.dispatchNativeEvents(
					batch,
					() => this.lifecycle === "open",
				);
			} catch (error) {
				// A malformed native envelope is an explicit transport failure, not
				// an unhandled rejection from the long-lived event lane.
				reportNativeEventError(error);
				return;
			}
		}
	}
}

function reportNativeEventError(error: unknown): void {
	try {
		const reportError = (
			globalThis as unknown as {
				reportError?: (error: unknown) => void;
			}
		).reportError;
		if (typeof reportError === "function") reportError(error);
		else console.error(error);
	} catch {
		// A broken diagnostic sink cannot restart or invalidate the native lane.
	}
}
