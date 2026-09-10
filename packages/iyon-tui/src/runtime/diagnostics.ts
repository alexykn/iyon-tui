import type {
	FramePhase,
	RuntimeErrorReporter,
	RuntimeFrameErrorCode,
	RuntimeFrameErrorRecord,
} from "./error-channel.ts";

const reporters = new WeakMap<object, RuntimeErrorReporter>();

export function setRuntimeErrorReporter(
	runtime: object,
	reporter: RuntimeErrorReporter | undefined,
): void {
	if (reporter === undefined) reporters.delete(runtime);
	else reporters.set(runtime, reporter);
}

export function reportRuntimeError(
	runtime: object,
	error: RuntimeFrameErrorRecord,
): void {
	const reporter = reporters.get(runtime);
	if (reporter === undefined) return;
	try {
		reporter(error);
	} catch {
		// Diagnostics must not alter accepted native state or barrier behavior.
	}
}

const PHASES: readonly string[] = [
	"structural",
	"content",
	"frame",
	"backend",
	"scheduler",
	"host",
];
const CODES: readonly string[] = [
	"FRAME_PREPARATION_FAILED",
	"BACKEND_NOT_READY",
	"BACKEND_IO_FAILED",
	"SURFACE_DESYNCHRONIZED",
	"LAYOUT_DID_NOT_CONVERGE",
	"INTERNAL_INVARIANT",
	"RUNTIME_POISONED",
	"SOURCE_WAKE_FAILED",
	"PROJECTION_FAILED",
	"LIMIT_EXCEEDED",
	"RETENTION_INCOMPATIBLE",
	"CONTENT_OPERATING_FAILED",
	"SOURCE_CLEANUP_PENDING",
	"ENVIRONMENT_WAKE_FAILED",
	"HOST_LOCK_POISONED",
];

/** Converts an untrusted N-API failure payload into the public diagnostic record. */
export function normalizeNativeFailure(
	hostId: string,
	failure: unknown,
): RuntimeFrameErrorRecord | undefined {
	if (failure === null || typeof failure !== "object") return undefined;
	const value = failure as Record<string, unknown>;
	const phase = value.phase;
	const code = value.code;
	const diagnostic = value.diagnostic;
	const retryable = value.retryable;
	const attemptedRevision = decimalBigInt(value.attempted_ui_revision);
	const attemptedEpoch = decimalBigInt(value.attempted_work_epoch);
	if (
		typeof phase !== "string" ||
		!isFramePhase(phase) ||
		typeof code !== "string" ||
		!isRuntimeFrameErrorCode(code) ||
		typeof diagnostic !== "string" ||
		typeof retryable !== "boolean" ||
		attemptedRevision === undefined ||
		attemptedEpoch === undefined
	)
		return undefined;
	return {
		hostId,
		attemptedEpoch,
		desiredRevision: attemptedRevision,
		phase,
		code,
		retryable,
		diagnostic,
	};
}

function isFramePhase(value: string): value is FramePhase {
	return PHASES.includes(value);
}

function isRuntimeFrameErrorCode(
	value: string,
): value is RuntimeFrameErrorCode {
	return CODES.includes(value);
}

function decimalBigInt(value: unknown): bigint | undefined {
	// N-API encodes u64 values as canonical decimal strings, never JS numbers.
	if (typeof value !== "string" || !/^(?:0|[1-9][0-9]{0,19})$/u.test(value))
		return undefined;
	const integer = BigInt(value);
	return integer <= 0xffff_ffff_ffff_ffffn ? integer : undefined;
}
