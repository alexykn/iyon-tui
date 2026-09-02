import { RuntimeErrorChannel, type FramePhase, type RuntimeFrameErrorCode, type RuntimeFrameErrorRecord } from "./error-channel.ts";

export interface NativeHostEpochs {
  readonly host_id: string | number;
  readonly desired_structural_revision: string | number;
  readonly visible_structural_revision: string | number;
  readonly visible_frame_revision: string | number;
  readonly pending_epoch: string | number;
  readonly committed_epoch: string | number;
}

export interface NativeHostCommit {
  readonly host_id: string | number;
  readonly committed_epoch: string | number;
  readonly visible_structural_revision: string | number;
}

export interface NativeHostDrainError {
  readonly host_id: string | number;
  readonly attempted_epoch: string | number;
  readonly desired_revision: string | number;
  readonly phase: string;
  readonly code: string;
  readonly retryable: boolean;
  readonly diagnostic: string;
}

export interface NativeHostDrainReport {
  readonly rearm: boolean;
  readonly waiting_for_presentation: boolean;
  readonly attempted: number;
  readonly commits: readonly NativeHostCommit[];
  readonly errors: readonly NativeHostDrainError[];
  readonly wake_epoch: string | number;
}

export interface NativeFrameHost {
  readonly epochs: () => NativeHostEpochs;
  readonly flushPendingHosts: (budget?: number, forceRetry?: boolean) => NativeHostDrainReport;
}

export interface RuntimeHostRegistration {
  readonly id: string;
  readonly token: object;
  readonly native: NativeFrameHost;
  markPending(): void;
  flush(): void;
  dispose(): void;
}

interface HostEntry {
  readonly registration: RuntimeHostRegistrationImpl;
  readonly errorChannel: WeakRef<RuntimeErrorChannel>;
  readonly onCommitted: (commit?: NativeHostCommit) => void;
}

const hostRegistrationFinalizer = new FinalizationRegistry<{
  readonly broker: WeakRef<EnvironmentWakeBroker>;
  readonly id: string;
}>(({ broker, id }) => broker.deref()?.unregisterId(id));

const DEFAULT_FLUSH_BUDGET = 32;
const MAX_EXPLICIT_DRAINS = 64;

export interface WakeBrokerCounters {
  pending_marks: number;
  wake_latch_wins: number;
  wake_already_latched: number;
  microtasks_queued: number;
  drains: number;
  hosts_attempted: number;
  frames_committed: number;
  automatic_errors: number;
  rearm_count: number;
  explicit_barriers: number;
  explicit_barrier_failures: number;
}

export interface WakeTraceEvent {
  readonly kind: "pending" | "drain" | "commit" | "error" | "rearm";
  readonly hostId?: string;
  readonly epoch?: bigint;
  readonly diagnostic?: string;
}

const wakeCounters: WakeBrokerCounters = {
  pending_marks: 0,
  wake_latch_wins: 0,
  wake_already_latched: 0,
  microtasks_queued: 0,
  drains: 0,
  hosts_attempted: 0,
  frames_committed: 0,
  automatic_errors: 0,
  rearm_count: 0,
  explicit_barriers: 0,
  explicit_barrier_failures: 0,
};
const wakeTrace: WakeTraceEvent[] = [];
const WAKE_TRACE_LIMIT = 256;

export function wakeBrokerCounterSnapshot(): WakeBrokerCounters {
  return { ...wakeCounters };
}

export function resetWakeBrokerCounters(): void {
  for (const key of Object.keys(wakeCounters) as Array<keyof WakeBrokerCounters>) wakeCounters[key] = 0;
  wakeTrace.length = 0;
}

export function wakeTraceSnapshot(): readonly WakeTraceEvent[] {
  return wakeTrace.slice();
}

function traceWake(event: WakeTraceEvent): void {
  if (typeof Bun !== "undefined" && Bun.env.PERF_RUNTIME_TRACE !== "1") return;
  wakeTrace.push(event);
  if (wakeTrace.length > WAKE_TRACE_LIMIT) wakeTrace.shift();
}

/**
 * One edge-triggered JavaScript wake broker per environment. Native epochs
 * decide whether work exists; this broker only chooses when to ask native for
 * a fair drain and routes structured outcomes without throwing from a
 * microtask.
 */
export class EnvironmentWakeBroker {
  private readonly hosts = new Map<string, HostEntry>();
  private readonly pending = new Set<string>();
  private microtaskQueued = false;
  private microtaskGeneration = 0;
  private pendingGeneration = 0;
  private draining = false;
  private fairCursor = 0;
  /** Polls an asynchronous terminal receipt without spinning microtasks. */
  private presentationPollTimer: ReturnType<typeof setTimeout> | undefined;

  constructor(
    private readonly budget = DEFAULT_FLUSH_BUDGET,
  ) {}

  register(
    native: NativeFrameHost,
    errorChannel: RuntimeErrorChannel,
    onCommitted: (commit?: NativeHostCommit) => void,
  ): RuntimeHostRegistration {
    const id = hostIdFor(native);
    if (this.hosts.has(id)) throw new Error(`duplicate native host identity ${id}`);
    const registration = new RuntimeHostRegistrationImpl(this, id, native);
    this.hosts.set(id, {
      registration,
      errorChannel: new WeakRef(errorChannel),
      onCommitted,
    });
    return registration;
  }

  /**
   * Queues one broker driver after an environment-owned mutation. Native
   * environment state already contains the complete affected-host set; one
   * driver is enough to drain it without mirroring subscriptions in JS.
   */
  markEnvironmentPending(): void {
    const entries = [...this.hosts.values()];
    for (let offset = 0; offset < entries.length; offset += 1) {
      const entry = entries[(this.fairCursor + offset) % entries.length]!;
      const native = entry.registration.nativeOrUndefined();
      if (native === undefined) {
        this.unregister(entry.registration);
        continue;
      }
      entry.registration.markPending();
      return;
    }
  }

  markPending(registration: RuntimeHostRegistrationImpl): void {
    if (!this.hosts.has(registration.id)) return;
    wakeCounters.pending_marks += 1;
    traceWake({ kind: "pending", hostId: registration.id });
    const latched = this.microtaskQueued || this.draining;
    this.pendingGeneration += 1;
    this.pending.add(registration.id);
    if (latched) wakeCounters.wake_already_latched += 1;
    else wakeCounters.wake_latch_wins += 1;
    this.scheduleMicrotask();
  }

  unregister(registration: RuntimeHostRegistrationImpl): void {
    hostRegistrationFinalizer.unregister(registration);
    this.unregisterId(registration.id);
  }

  /** @internal FinalizationRegistry callback for abandoned native hosts. */
  unregisterId(id: string): void {
    this.pending.delete(id);
    this.hosts.delete(id);
    if (this.pending.size === 0) {
      this.cancelPresentationPoll();
      this.microtaskQueued = false;
      this.microtaskGeneration += 1;
    }
  }

  /** Explicit read-your-writes barrier for one host. */
  flush(registration: RuntimeHostRegistrationImpl): void {
    if (!this.hosts.has(registration.id)) return;
    wakeCounters.explicit_barriers += 1;
    this.pending.add(registration.id);
    const capturedEpoch = readEpochs(registration.native).pending;
    this.cancelQueuedMicrotask();
    let lastReport = emptyReport();
    for (let attempt = 0; attempt < MAX_EXPLICIT_DRAINS; attempt += 1) {
      const report = this.drain(true, registration.id);
      lastReport = report;
      try {
        this.throwHostError(registration.id);
      } catch (error) {
        this.scheduleRemainingAfterBarrier(report);
        throw error;
      }
      const epochs = readEpochs(registration.native);
      if (epochs.committed >= capturedEpoch) {
        this.pending.delete(registration.id);
        this.scheduleRemainingAfterBarrier(report);
        return;
      }
      if (!report.rearm && report.attempted === 0) break;
    }
    this.scheduleRemainingAfterBarrier(lastReport);
    const epochs = readEpochs(registration.native);
    const record = {
      hostId: registration.id,
      attemptedEpoch: epochs.pending,
      desiredRevision: epochs.desired,
      phase: "frame" as const,
      code: "FRAME_PREPARATION_FAILED" as const,
      retryable: true,
      diagnostic: "explicit frame barrier did not reach the requested host epoch",
    };
    const entry = this.hosts.get(registration.id);
    entry?.errorChannel.deref()?.accept(record);
    try {
      entry?.errorChannel.deref()?.throwPending(registration.id);
    } catch (error) {
      wakeCounters.explicit_barrier_failures += 1;
      throw error;
    }
    wakeCounters.explicit_barrier_failures += 1;
    throw new Error(record.diagnostic);
  }

  private scheduleRemainingAfterBarrier(report: NativeHostDrainReport): void {
    if (report.rearm && this.pending.size > 0) this.scheduleMicrotask();
  }

  private scheduleMicrotask(): void {
    if (this.microtaskQueued || this.draining) return;
    wakeCounters.microtasks_queued += 1;
    this.microtaskQueued = true;
    const generation = ++this.microtaskGeneration;
    queueMicrotask(() => {
      if (!this.microtaskQueued || generation !== this.microtaskGeneration) return;
      this.microtaskQueued = false;
      this.drainAutomatically();
    });
  }

  private cancelQueuedMicrotask(): void {
    if (!this.microtaskQueued) return;
    this.microtaskQueued = false;
    this.microtaskGeneration += 1;
  }

  private drainAutomatically(): void {
    if (this.draining || this.pending.size === 0) return;
    // A false rearm is authoritative: it includes retry-blocked hosts after a
    // failed automatic attempt. Do not infer runnable work from the unchanged
    // pending epoch or a persistent failure will spin microtasks forever. A
    // new edge arriving during the native call is the one exception and is
    // detected by the local pending generation recheck.
    const generation = this.pendingGeneration;
    try {
      const report = this.drain(false);
      if ((report.rearm || this.pendingGeneration !== generation) && this.pending.size > 0) {
        this.scheduleMicrotask();
      } else if (report.waiting_for_presentation === true) {
        // A real terminal backend acknowledges a submitted frame through an
        // asynchronous receipt. It is pending work, but not runnable work;
        // polling it with another microtask would create a busy loop. Yield to
        // the event loop and ask the broker to poll once the receipt can settle.
        this.schedulePresentationPoll();
      }
    } catch (error) {
      // The automatic boundary is deliberately non-throwing. Preserve the
      // pending epoch and route any unexpected broker failure into the same
      // host error channel; an explicit barrier can retry it later.
      const hostId = [...this.pending][0];
      if (hostId !== undefined) {
        const entry = this.hosts.get(hostId);
        const diagnostic = error instanceof Error ? error.message : String(error);
        entry?.errorChannel.deref()?.accept(fallbackError(hostId, diagnostic));
      }
    }
  }

  private drain(forceRetry: boolean, preferredHostId?: string): NativeHostDrainReport {
    if (this.draining) return emptyReport();
    wakeCounters.drains += 1;
    traceWake({ kind: "drain", hostId: preferredHostId });
    this.draining = true;
    try {
      const driver = this.chooseDriver(preferredHostId);
      if (driver === undefined) {
        this.pending.clear();
        return emptyReport();
      }
      const native = driver.nativeOrUndefined();
      if (native === undefined) {
        this.unregister(driver);
        return { ...emptyReport(), rearm: this.pending.size > 0 };
      }
      let report: NativeHostDrainReport;
      try {
        report = native.flushPendingHosts(this.budget, forceRetry);
      } catch (error) {
        const record = fallbackError(driver.id, error instanceof Error ? error.message : String(error));
        this.hosts.get(driver.id)?.errorChannel.deref()?.accept(record);
        return { ...emptyReport(), errors: [toNativeError(record)] };
      }
      wakeCounters.hosts_attempted += report.attempted;
      if (report.rearm) {
        wakeCounters.rearm_count += 1;
        traceWake({ kind: "rearm" });
      }
      this.consumeReport(report, !forceRetry);
      this.refreshPendingHints(report.rearm ? driver.id : undefined);
      return report;
    } finally {
      this.draining = false;
    }
  }

  private consumeReport(report: NativeHostDrainReport, automatic: boolean): void {
    for (const error of report.errors) {
      const id = String(error.host_id);
      if (automatic) wakeCounters.automatic_errors += 1;
      const record = fromNativeError(error);
      traceWake({ kind: "error", hostId: id, epoch: record.attemptedEpoch, diagnostic: record.diagnostic });
      this.hosts.get(id)?.errorChannel.deref()?.accept(record);
    }
    for (const revisioned of report.commits) {
      const { host_id } = revisioned;
      const id = String(host_id);
      const entry = this.hosts.get(id);
      if (entry === undefined) continue;
      try {
        wakeCounters.frames_committed += 1;
        traceWake({ kind: "commit", hostId: id });
        entry.onCommitted(revisioned);
        entry.errorChannel.deref()?.markCommitted(id);
        this.pending.delete(id);
      } catch (error) {
        entry.errorChannel.deref()?.accept(fallbackError(id, error instanceof Error ? error.message : String(error)));
      }
    }
  }

  private throwHostError(hostId: string): void {
    try {
      this.hosts.get(hostId)?.errorChannel.deref()?.throwPending(hostId);
    } catch (error) {
      wakeCounters.explicit_barrier_failures += 1;
      throw error;
    }
  }

  private schedulePresentationPoll(): void {
    if (this.presentationPollTimer !== undefined) return;
    this.presentationPollTimer = setTimeout(() => {
      this.presentationPollTimer = undefined;
      if (this.pending.size > 0) this.scheduleMicrotask();
    }, 1);
  }

  private cancelPresentationPoll(): void {
    if (this.presentationPollTimer === undefined) return;
    clearTimeout(this.presentationPollTimer);
    this.presentationPollTimer = undefined;
  }

  private refreshPendingHints(retainedDriverId?: string): void {
    for (const id of [...this.pending]) {
      if (id === retainedDriverId) continue;
      const entry = this.hosts.get(id);
      if (entry === undefined) {
        this.pending.delete(id);
        continue;
      }
      const native = entry.registration.nativeOrUndefined();
      if (native === undefined) {
        this.unregister(entry.registration);
        continue;
      }
      const epochs = readEpochs(native);
      if (epochs.pending === epochs.committed) {
        this.pending.delete(id);
      }
    }
  }

  private chooseDriver(preferredHostId?: string): RuntimeHostRegistrationImpl | undefined {
    if (preferredHostId !== undefined) {
      const preferred = this.hosts.get(preferredHostId)?.registration;
      if (preferred !== undefined) return preferred;
    }
    const entries = [...this.pending]
      .map((id) => this.hosts.get(id)?.registration)
      .filter((registration): registration is RuntimeHostRegistrationImpl => registration !== undefined);
    if (entries.length === 0) return undefined;
    const driver = entries[this.fairCursor % entries.length]!;
    this.fairCursor = (this.fairCursor + 1) % entries.length;
    return driver;
  }
}

class RuntimeHostRegistrationImpl implements RuntimeHostRegistration {
  readonly token = {};
  private readonly nativeRef: WeakRef<NativeFrameHost>;

  constructor(
    private readonly broker: EnvironmentWakeBroker,
    readonly id: string,
    native: NativeFrameHost,
  ) {
    this.nativeRef = new WeakRef(native);
    hostRegistrationFinalizer.register(native, { broker: new WeakRef(broker), id }, this);
  }

  get native(): NativeFrameHost {
    const native = this.nativeRef.deref();
    if (native === undefined) throw new Error("native host is no longer live");
    return native;
  }

  nativeOrUndefined(): NativeFrameHost | undefined { return this.nativeRef.deref(); }
  markPending(): void { this.broker.markPending(this); }
  flush(): void { this.broker.flush(this); }
  dispose(): void { this.broker.unregister(this); }
}

function hostIdFor(native: NativeFrameHost): string {
  return readEpochs(native).hostId.toString();
}

function readEpochs(native: NativeFrameHost): {
  readonly hostId: bigint;
  readonly desired: bigint;
  readonly visible: bigint;
  readonly pending: bigint;
  readonly committed: bigint;
} {
  const value = native.epochs();
  return {
    hostId: toBigInt(value.host_id),
    desired: toBigInt(value.desired_structural_revision),
    visible: toBigInt(value.visible_frame_revision),
    pending: toBigInt(value.pending_epoch),
    committed: toBigInt(value.committed_epoch),
  };
}

function toBigInt(value: string | number): bigint {
  if (typeof value === "number") {
    if (!Number.isSafeInteger(value) || value < 0) throw new TypeError("native epoch must be a non-negative safe integer");
    return BigInt(value);
  }
  if (!/^\d+$/u.test(value)) throw new TypeError("native epoch must be a decimal integer string");
  return BigInt(value);
}

function fallbackError(hostId: string, diagnostic: string): RuntimeFrameErrorRecord {
  return {
    hostId,
    attemptedEpoch: 0n,
    desiredRevision: 0n,
    phase: "frame",
    code: "FRAME_PREPARATION_FAILED",
    retryable: true,
    diagnostic,
  };
}

function fromNativeError(error: NativeHostDrainError): RuntimeFrameErrorRecord {
  const code = frameCode(error.code);
  return {
    hostId: String(error.host_id),
    attemptedEpoch: toBigInt(error.attempted_epoch),
    desiredRevision: toBigInt(error.desired_revision),
    phase: framePhase(error.phase),
    code,
    retryable: code === "INTERNAL_INVARIANT" ? false : error.retryable,
    diagnostic: error.diagnostic,
  };
}

function toNativeError(error: RuntimeFrameErrorRecord): NativeHostDrainError {
  return {
    host_id: error.hostId,
    attempted_epoch: error.attemptedEpoch.toString(),
    desired_revision: error.desiredRevision.toString(),
    phase: error.phase,
    code: error.code,
    retryable: error.retryable,
    diagnostic: error.diagnostic,
  };
}

function framePhase(value: string): FramePhase {
  if (value === "structural" || value === "backend") return value;
  return "frame";
}

function frameCode(value: string): RuntimeFrameErrorCode {
  switch (value) {
    case "BACKEND_NOT_READY":
    case "BACKEND_IO_FAILED":
    case "SURFACE_DESYNCHRONIZED":
    case "LAYOUT_DID_NOT_CONVERGE":
    case "INTERNAL_INVARIANT":
    case "RUNTIME_POISONED":
    case "FRAME_PREPARATION_FAILED":
      return value;
    default:
      // A native report with an unknown code is a protocol/invariant failure;
      // do not silently downgrade it to an ordinary retryable frame error.
      return "INTERNAL_INVARIANT";
  }
}

function emptyReport(): NativeHostDrainReport {
  return {
    rearm: false,
    waiting_for_presentation: false,
    attempted: 0,
    commits: [],
    errors: [],
    wake_epoch: "0",
  };
}
