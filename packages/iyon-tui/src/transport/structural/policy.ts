/** Native axis-constructor thresholds selected by PERF-11.8. */
export const NATIVE_SMALL_AXIS_ARITY_MAX = 4;
export const NATIVE_BUILDER_MAX_CHILDREN = 524_288;

/**
 * PRE-V5-R0: the retained structural path is the single production
 * architecture, so it carries no refusal budgets. A large or deep tree costs
 * more retained work; it never selects a previous-generation execution path.
 * True native limits (axis child count, text bytes) fail explicitly with a
 * native status inside the retained transaction.
 */
export const MAX_RETAINED_NEW_NODES = Number.POSITIVE_INFINITY;
export const MAX_RETAINED_DEPTH = Number.POSITIVE_INFINITY;

/**
 * PRE-V5-R0: single enforcement point is the native limit itself
 * (`MAX_AXIS_CHILD_COUNT = 524_288`). The retained scratch grows to the
 * requested arity; beyond the native limit the constructor returns an
 * explicit failure status instead of routing to another architecture.
 */
export const MAX_DIRECT_AXIS_REFS = 524_288;

/**
 * PRE-V5-R0: grid word scratch grows to the requested word count. There is
 * no TypeScript-side refusal cap; allocation and native validation are the
 * only limits, and both fail explicitly inside the retained transaction.
 */
export const MAX_DIRECT_GRID_WORDS = Number.POSITIVE_INFINITY;

/**
 * PRE-V5-R0: text/diff payload scratch grows to the requested byte count, up
 * to the native `MAX_NEW_TEXT_BYTES` (16 MiB) limit enforced by the
 * constructors themselves. Exceeding it fails explicitly; it never selects
 * another transport.
 */
export const MAX_DIRECT_TEXT_BYTES = 16 * 1024 * 1024;
export const MAX_DIRECT_DIFF_WORDS = Number.POSITIVE_INFINITY;
export const MAX_DIRECT_DIFF_BYTES = 16 * 1024 * 1024;
