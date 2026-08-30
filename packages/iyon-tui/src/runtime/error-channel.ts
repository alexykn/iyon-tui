import { TuiError } from "../api/errors.ts";

export type FramePhase = "structural" | "frame" | "backend";

export type RuntimeFrameErrorCode =
  | "FRAME_PREPARATION_FAILED"
  | "BACKEND_NOT_READY"
  | "BACKEND_IO_FAILED"
  | "SURFACE_DESYNCHRONIZED"
  | "LAYOUT_DID_NOT_CONVERGE"
  | "INTERNAL_INVARIANT"
  | "RUNTIME_POISONED";

/** Structured native failure retained until an explicit barrier observes it. */
export interface RuntimeFrameErrorRecord {
  readonly hostId: string;
  readonly attemptedEpoch: bigint;
  readonly desiredRevision: bigint;
  readonly phase: FramePhase;
  readonly code: RuntimeFrameErrorCode;
  readonly retryable: boolean;
  readonly diagnostic: string;
}

export type RuntimeErrorReporter = (error: RuntimeFrameErrorRecord) => boolean | void;

interface ErrorGlobals {
  reportError?: (error: unknown) => void;
}

/**
 * Host-scoped error channel for automatic environment drains. Reporting is
 * best-effort and never allowed to throw into a microtask; explicit barriers
 * turn the stored record back into a deterministic TuiError.
 */
export class RuntimeErrorChannel {
  private readonly latest = new Map<string, RuntimeFrameErrorRecord>();
  private readonly reported = new Set<string>();
  private reporter: RuntimeErrorReporter | undefined;

  constructor(reporter?: RuntimeErrorReporter) {
    this.reporter = reporter;
  }

  setReporter(reporter: RuntimeErrorReporter | undefined): void {
    this.reporter = reporter;
  }

  accept(record: RuntimeFrameErrorRecord): void {
    const key = errorKey(record);
    const previous = this.latest.get(record.hostId);
    if (previous !== undefined && errorKey(previous) !== key) this.reported.delete(errorKey(previous));
    this.latest.set(record.hostId, record);
    if (this.reported.has(key)) return;
    this.reported.add(key);
    this.reportSafely(record);
  }

  latestFor(hostId: string): RuntimeFrameErrorRecord | undefined {
    return this.latest.get(hostId);
  }

  /** Clears an error once a later frame for this host commits. */
  markCommitted(hostId: string): void {
    const record = this.latest.get(hostId);
    if (record === undefined) return;
    this.latest.delete(hostId);
    this.reported.delete(errorKey(record));
  }

  /** Marks a stored error observed and throws it at an explicit barrier. */
  throwPending(hostId: string): void {
    const record = this.latest.get(hostId);
    if (record === undefined) return;
    this.latest.delete(hostId);
    this.reported.delete(errorKey(record));
    throw new TuiError(
      "runtime",
      record.diagnostic,
      record.code,
      {
        hostId: record.hostId,
        attemptedEpoch: record.attemptedEpoch.toString(),
        desiredRevision: record.desiredRevision.toString(),
        phase: record.phase,
        retryable: record.retryable,
      },
    );
  }

  clear(hostId: string): void {
    const record = this.latest.get(hostId);
    this.latest.delete(hostId);
    if (record !== undefined) this.reported.delete(errorKey(record));
  }

  private reportSafely(record: RuntimeFrameErrorRecord): void {
    try {
      if (this.reporter !== undefined && this.reporter(record) === true) return;
      const reportError = (globalThis as unknown as ErrorGlobals).reportError;
      if (typeof reportError === "function") reportError(toReportedError(record));
      else console.error(toReportedError(record));
    } catch (error) {
      // Reporting must not make the automatic drain throw. If the configured
      // reporter fails, use the platform reporter once more when available;
      // otherwise make the failure visible through the package's diagnostic
      // fallback while leaving frame/retry state untouched.
      try {
        const reportError = (globalThis as unknown as ErrorGlobals).reportError;
        if (typeof reportError === "function") reportError(error);
        else console.error(error);
      } catch {
        // A broken diagnostic sink cannot change frame or retry state.
      }
    }
  }
}

function errorKey(record: RuntimeFrameErrorRecord): string {
  return `${record.hostId}:${record.attemptedEpoch.toString()}:${record.desiredRevision.toString()}:${record.phase}:${record.code}`;
}

function toReportedError(record: RuntimeFrameErrorRecord): Error {
  const error = new Error(record.diagnostic);
  error.name = `TuiRuntimeError/${record.code}`;
  return error;
}
