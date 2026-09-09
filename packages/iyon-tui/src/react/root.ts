import { createElement, type ReactNode } from "react";
import Reconciler from "react-reconciler";
import type { TuiRuntime } from "../runtime/runtime.ts";
import type { NativeTuiHostContract } from "../transport/native/addon.ts";
import { type CommitCoordinator, RootContainer } from "./commit.ts";
import { hostConfig } from "./host-config.ts";
import { nativeHostForReact } from "./host-registry.ts";

const activeRoots = new WeakSet<object>();
const closedHosts = new WeakSet<object>();

export interface ReactCommit {
	readonly revision: number;
	readonly accepted: true;
}

export class ReactFrameBarrierError extends Error {
	readonly code = "T3_FRAME_BARRIER_UNREALIZED" as const;

	constructor() {
		super(
			"React UI acceptance succeeded, but T3 does not implement a terminal presentation barrier yet",
		);
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
	private closed = false;
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
	}

	get faulted(): boolean {
		return this.coordinator.isFaulted;
	}

	render(children: ReactNode): Promise<ReactCommit> {
		if (this.closed) return Promise.reject(new Error("React root is closed"));
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
		if (this.closed) throw new Error("React root is closed");
		return createElement(
			"portal",
			{ key, portalOwner: this.container },
			children,
		);
	}

	whenVisible(_revision?: number): Promise<void> {
		return Promise.reject(new ReactFrameBarrierError());
	}

	whenContentVisible(_revision?: number): Promise<void> {
		return Promise.reject(new ReactFrameBarrierError());
	}

	async unmount(): Promise<ReactCommit> {
		return this.render(null);
	}

	close(): void {
		if (this.closed) return;
		let failure: unknown;
		if (!this.reactCleanupAttempted) {
			this.reactCleanupAttempted = true;
			try {
				if (this.coordinator.isFaulted) this.coordinator.beginCleanup();
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
			this.closed = true;
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
}
