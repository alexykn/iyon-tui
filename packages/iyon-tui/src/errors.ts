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
      return "validation";
    case "ION_CANCELLED":
      return "cancelled";
    case "ION_CLOSED":
      return "terminal";
    default:
      return "runtime";
  }
}
