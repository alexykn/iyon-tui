export type TuiErrorCategory =
  | "invalid-handle"
  | "disposed-handle"
  | "validation"
  | "terminal"
  | "runtime"
  | "projection"
  | "stream"
  | "cancelled";

export class TuiError extends Error {
  readonly name = "TuiError";

  constructor(
    readonly category: TuiErrorCategory,
    message: string,
    readonly nativeCode?: string,
    readonly context?: Readonly<Record<string, unknown>>,
  ) {
    super(message);
  }
}

export function tuiError(
  category: TuiErrorCategory,
  message: string,
  context?: Readonly<Record<string, unknown>>,
): TuiError {
  return new TuiError(category, message, undefined, context);
}

export function asTuiError(error: unknown): TuiError {
  if (error instanceof TuiError) {
    return error;
  }
  if (error instanceof Error) {
    const match = /^(ION_[A-Z_]+):\s*(.*)$/s.exec(error.message);
    const nativeCode = match?.[1];
    const message = match?.[2] ?? error.message;
    const category = categoryForNativeCode(nativeCode);
    return new TuiError(category, message, nativeCode);
  }
  return new TuiError("runtime", String(error));
}

export function isTuiError(error: unknown): error is TuiError {
  return error instanceof TuiError;
}

export function isTuiCancelledError(error: unknown): boolean {
  return isTuiError(error) && error.category === "cancelled";
}

function categoryForNativeCode(code: string | undefined): TuiErrorCategory {
  switch (code) {
    case "ION_INVALID_HANDLE":
      return "invalid-handle";
    case "ION_DISPOSED_HANDLE":
      return "disposed-handle";
    case "ION_INVALID_INPUT":
    case "ION_INVALID_ARGUMENT":
      return "validation";
    case "ION_WRONG_ENVIRONMENT":
    case "ION_WRONG_HOST":
    case "ION_STALE_HANDLE":
    case "ION_PORT_MOUNTED":
    case "ION_PORT_DISPOSED":
    case "ION_CONNECTOR_DISPOSING":
    case "ION_CONNECTOR_DISPOSED":
      return "invalid-handle";
    case "ION_SOURCE_IN_USE":
    case "ION_SOURCE_DISPOSED":
    case "ION_SOURCE_SEALED":
    case "ION_SOURCE_ALREADY_SEALED":
    case "ION_STALE_SOURCE":
    case "ION_INVALID_UTF8":
    case "ION_INVALID_RANGE":
    case "ION_UNKNOWN_ANNOTATION_KIND":
    case "ION_INVALID_ANNOTATION_PAYLOAD":
    case "ION_LIMIT_EXCEEDED":
    case "ION_PAYLOAD_TOO_LARGE":
    case "ION_SOURCE_RETENTION_OVERFLOW":
    case "ION_PORT_IN_USE":
    case "ION_CONTENT_FAMILY_MISMATCH":
    case "ION_DUPLICATE_CONTENT_PORT_ATTACHMENT":
    case "ION_UNSUPPORTED_CONTENT_PORT_ATTACHMENT":
    case "ION_INVALID_FUNNEL":
    case "ION_CONNECTOR_NOT_MEMBER":
      return "validation";
    case "ION_PROJECTION_FAILED":
      return "projection";
    case "ION_HOST_DISPOSED":
      return "terminal";
    case "ION_INTERNAL_INVARIANT":
    case "ION_RUNTIME_POISONED":
    case "ION_INTERNAL_PANIC":
      return "runtime";
    case "ION_CANCELLED":

      return "cancelled";
    case "ION_CLOSED":
      return "terminal";
    default:
      return "runtime";
  }
}
