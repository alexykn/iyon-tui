import { RuntimeErrorChannel, type FramePhase, type RuntimeFrameErrorCode, type RuntimeFrameErrorRecord } from "./error-channel.ts";

export interface NativeHostEpochs {
  readonly host_id: string | number;
  readonly desired_structural_revision: string | number;
  readonly visible_frame_revision: string | number;
  readonly pending_epoch: string | number;
  readonly committed_epoch: string | number;
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
  readonly attempted: number;
  readonly committed_hosts: readonly (string | number)[];
  readonly errors: readonly NativeHostDrainError[];
  readonly wake_epoch: string | number;
}

export interface NativeFrameHost {
  readonly epochs?: () => NativeHostEpochs;
  readonly flushPendingHosts?: (budget?: number, forceRetry?: boolean) => NativeHostDrainReport;
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
  readonly errorChannel: RuntimeErrorChannel;
  readonly onCommitted: () => void;
}

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

  constructor(
    private readonly budget = DEFAULT_FLUSH_BUDGET,
  ) {}

  register(
    native: NativeFrameHost,
    errorChannel: RuntimeErrorChannel,
    onCommitted: () => void,
  ): RuntimeHostRegistration {
    const id = hostIdFor(native, this.hosts.size + 1);
    const registration = new RuntimeHostRegistrationImpl(this, id, native);
    this.hosts.set(id, { registration, errorChannel, onCommitted });
    return registration;
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
    this.pending.delete(registration.id);
    this.hosts.delete(registration.id);
    if (this.pending.size === 0) {
      this.microtaskQueued = false;
      this.microtaskGeneration += 1;
    }
  }

  /** Explicit read-your-writes barrier for one host. */
  flush(registration: RuntimeHostRegistrationImpl): void {
    if (!this.hosts.has(registration.id)) return;
    wakeCounters.explicit_barriers += 1;
    this.pending.add(registration.id);
    const capturedEpoch = readEpochs(registration.native)?.pending;
    this.cancelQueuedMicrotask();
    for (let attempt = 0; attempt < MAX_EXPLICIT_DRAINS; attempt += 1) {
      const report = this.drain(true, registration.id);
      this.throwHostError(registration.id);
      const epochs = readEpochs(registration.native);
      if (epochs === undefined
        || capturedEpoch === undefined
        || epochs.committed >= capturedEpoch) {
        this.pending.delete(registration.id);
        return;
      }
      if (!report.rearm && report.attempted === 0) break;
    }
    const epochs = readEpochs(registration.native);
    const record = epochs === undefined
      ? fallbackError(registration.id, "unable to read native host epochs")
      : {
        hostId: registration.id,
        attemptedEpoch: epochs.pending,
        desiredRevision: epochs.desired,
        phase: "frame" as const,
        code: "FRAME_PREPARATION_FAILED" as const,
        retryable: true,
        diagnostic: "explicit frame barrier did not reach the requested host epoch",
      };
    const entry = this.hosts.get(registration.id);
    entry?.errorChannel.accept(record);
    try {
      entry?.errorChannel.throwPending(registration.id);
    } catch (error) {
      wakeCounters.explicit_barrier_failures += 1;
      throw error;
    }
    wakeCounters.explicit_barrier_failures += 1;
    throw new Error(record.diagnostic);
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
      }
    } catch (error) {
      // The automatic boundary is deliberately non-throwing. Preserve the
      // pending epoch and route any unexpected broker failure into the same
      // host error channel; an explicit barrier can retry it later.
      const hostId = [...this.pending][0];
      if (hostId !== undefined) {
        const entry = this.hosts.get(hostId);
        const diagnostic = error instanceof Error ? error.message : String(error);
        entry?.errorChannel.accept(fallbackError(hostId, diagnostic));
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
      const flush = driver.native.flushPendingHosts;
      if (flush === undefined) {
        // Older addons use the compatibility synchronous host path. There is
        // no pending native epoch to broker in that mode, so avoid queuing an
        // unresolvable retry loop.
        this.pending.delete(driver.id);
        return emptyReport();
      }
      let report: NativeHostDrainReport;
      try {
        report = flush.call(driver.native, this.budget, forceRetry);
      } catch (error) {
        const record = fallbackError(driver.id, error instanceof Error ? error.message : String(error));
        this.hosts.get(driver.id)?.errorChannel.accept(record);
        return { ...emptyReport(), errors: [toNativeError(record)] };
      }
      wakeCounters.hosts_attempted += report.attempted;
      if (report.rearm) {
        wakeCounters.rearm_count += 1;
        traceWake({ kind: "rearm" });
      }
      this.consumeReport(report, !forceRetry);
      this.refreshPendingHints();
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
      this.hosts.get(id)?.errorChannel.accept(record);
    }
    for (const rawId of report.committed_hosts) {
      const id = String(rawId);
      const entry = this.hosts.get(id);
      if (entry === undefined) continue;
      try {
        wakeCounters.frames_committed += 1;
        traceWake({ kind: "commit", hostId: id });
        entry.onCommitted();
        entry.errorChannel.markCommitted(id);
      } catch (error) {
        entry.errorChannel.accept(fallbackError(id, error instanceof Error ? error.message : String(error)));
      }
    }
  }

  private throwHostError(hostId: string): void {
    try {
      this.hosts.get(hostId)?.errorChannel.throwPending(hostId);
    } catch (error) {
      wakeCounters.explicit_barrier_failures += 1;
      throw error;
    }
  }

  private refreshPendingHints(): void {
    for (const id of [...this.pending]) {
      const entry = this.hosts.get(id);
      if (entry === undefined) {
        this.pending.delete(id);
        continue;
      }
      const epochs = readEpochs(entry.registration.native);
      if (epochs !== undefined && epochs.pending === epochs.committed) {
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

  constructor(
    private readonly broker: EnvironmentWakeBroker,
    readonly id: string,
    readonly native: NativeFrameHost,
  ) {}

  markPending(): void { this.broker.markPending(this); }
  flush(): void { this.broker.flush(this); }
  dispose(): void { this.broker.unregister(this); }
}

function hostIdFor(native: NativeFrameHost, fallback: number): string {
  const epochs = readEpochs(native);
  return epochs?.hostId.toString() ?? `js-${fallback}`;
}

function readEpochs(native: NativeFrameHost): {
  readonly hostId: bigint;
  readonly desired: bigint;
  readonly visible: bigint;
  readonly pending: bigint;
  readonly committed: bigint;
} | undefined {
  if (native.epochs === undefined) return undefined;
  try {
    const value = native.epochs();
    return {
      hostId: toBigInt(value.host_id),
      desired: toBigInt(value.desired_structural_revision),
      visible: toBigInt(value.visible_frame_revision),
      pending: toBigInt(value.pending_epoch),
      committed: toBigInt(value.committed_epoch),
    };
  } catch {
    return undefined;
  }
}

function toBigInt(value: string | number): bigint {
  return typeof value === "number" ? BigInt(value) : BigInt(value);
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
  return {
    hostId: String(error.host_id),
    attemptedEpoch: toBigInt(error.attempted_epoch),
    desiredRevision: toBigInt(error.desired_revision),
    phase: framePhase(error.phase),
    code: frameCode(error.code),
    retryable: error.retryable,
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
      return "FRAME_PREPARATION_FAILED";
  }
}

function emptyReport(): NativeHostDrainReport {
  return { rearm: false, attempted: 0, committed_hosts: [], errors: [], wake_epoch: "0" };
}
