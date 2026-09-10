export type FramePhase =
	| "structural"
	| "content"
	| "frame"
	| "backend"
	| "scheduler"
	| "host";

export type RuntimeFrameErrorCode =
	| "FRAME_PREPARATION_FAILED"
	| "BACKEND_NOT_READY"
	| "BACKEND_IO_FAILED"
	| "SURFACE_DESYNCHRONIZED"
	| "LAYOUT_DID_NOT_CONVERGE"
	| "INTERNAL_INVARIANT"
	| "RUNTIME_POISONED"
	| "SOURCE_WAKE_FAILED"
	| "PROJECTION_FAILED"
	| "LIMIT_EXCEEDED"
	| "RETENTION_INCOMPATIBLE"
	| "CONTENT_OPERATING_FAILED"
	| "SOURCE_CLEANUP_PENDING"
	| "ENVIRONMENT_WAKE_FAILED"
	| "HOST_LOCK_POISONED";

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

export type RuntimeErrorReporter = (
	error: RuntimeFrameErrorRecord,
) => boolean | void;
