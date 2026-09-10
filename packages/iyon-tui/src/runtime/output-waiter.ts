import { tuiError } from "../api/errors.ts";

export interface NativeOutput {
	readonly route_id: string;
	readonly payload?: string | null;
}

interface PendingWaiter {
	readonly signal?: AbortSignal;
	readonly resolve: (output: NativeOutput | null) => void;
	readonly reject: (error: unknown) => void;
	onAbort?: () => void;
}

/**
 * Owns the single native output consumer for one host.
 *
 * Native output is a consumable stream, so a promise cannot be shared between
 * callers: doing so delivers one frame to every concurrent waiter.  This
 * owner keeps callers in FIFO order, retains one output when its waiter is
 * cancelled, and never starts a second native consumer while one is active.
 */
export class OutputWaitOwner {
	private readonly pending: PendingWaiter[] = [];
	private readonly buffered: NativeOutput[] = [];
	private pumpActive = false;
	private closed = false;
	private failure: unknown;

	constructor(
		private readonly waitForNativeOutput: () =>
			| Promise<NativeOutput | null>
			| NativeOutput
			| null,
	) {}

	wait(signal?: AbortSignal): Promise<NativeOutput | null> {
		if (signal?.aborted)
			return Promise.reject(
				tuiError("cancelled", "TUI event wait was cancelled"),
			);
		if (this.closed) return Promise.resolve(null);
		if (this.failure !== undefined) return Promise.reject(this.failure);

		const buffered = this.buffered.shift();
		if (buffered !== undefined) return Promise.resolve(buffered);

		return new Promise<NativeOutput | null>((resolve, reject) => {
			const waiter: PendingWaiter = { signal, resolve, reject };
			if (signal !== undefined) {
				waiter.onAbort = () => {
					const index = this.pending.indexOf(waiter);
					if (index === -1) return;
					this.pending.splice(index, 1);
					reject(tuiError("cancelled", "TUI event wait was cancelled"));
				};
				signal.addEventListener("abort", waiter.onAbort, { once: true });
			}
			this.pending.push(waiter);
			this.pump();
		});
	}

	/** Resolve live callers and stop admitting output before the native host is closed. */
	close(): void {
		if (this.closed) return;
		this.closed = true;
		const pending = this.pending.splice(0);
		for (const waiter of pending) {
			this.removeAbortListener(waiter);
			waiter.resolve(null);
		}
		this.buffered.length = 0;
	}

	private pump(): void {
		if (this.pumpActive || this.closed || this.failure !== undefined) return;
		if (this.pending.length === 0) return;
		this.pumpActive = true;
		Promise.resolve()
			.then(() => this.waitForNativeOutput())
			.then(
				(output) => {
					this.pumpActive = false;
					if (this.closed) return;
					const waiter = this.pending.shift();
					if (waiter !== undefined) {
						this.removeAbortListener(waiter);
						waiter.resolve(output);
					} else if (output !== null) {
						this.buffered.push(output);
					}
					if (output === null) {
						this.closed = true;
						this.resolveRemaining(null);
						return;
					}
					this.pump();
				},
				(error) => {
					this.pumpActive = false;
					if (this.closed) return;
					this.failure = error;
					const pending = this.pending.splice(0);
					for (const waiter of pending) {
						this.removeAbortListener(waiter);
						waiter.reject(error);
					}
				},
			);
	}

	private resolveRemaining(output: NativeOutput | null): void {
		const pending = this.pending.splice(0);
		for (const waiter of pending) {
			this.removeAbortListener(waiter);
			waiter.resolve(output);
		}
	}

	private removeAbortListener(waiter: PendingWaiter): void {
		if (waiter.signal !== undefined && waiter.onAbort !== undefined)
			waiter.signal.removeEventListener("abort", waiter.onAbort);
	}
}
