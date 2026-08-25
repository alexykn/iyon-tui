/**
 * Cold/new-graph routing thresholds selected by PERF-11.8. Small arities use
 * one generated constructor; larger supported axes use the typed native
 * builder; oversized or unsupported graphs remain on V4/direct fallback.
 */
export const NATIVE_SMALL_AXIS_ARITY_MAX = 4;
export const NATIVE_BUILDER_MAX_CHILDREN = 524_288;
export const NATIVE_COLD_MAX_NODES = 524_288;
export const NATIVE_COLD_MAX_DEPTH = 128;
export const NATIVE_TEXT_MAX_BYTES = 16_777_216;

/**
 * PERF-12 T6 retained-work budgets (§50). These are not public semantic
 * limits: exceeding either returns FAST_FALLBACK and the caller routes to the
 * complete cold path. Final values come from realistic traces at T15.
 */
export const MAX_RETAINED_NEW_NODES = 512;
export const MAX_RETAINED_DEPTH = 256;

/**
 * PERF-12 T8 (§50): retained cap on children transported through ONE
 * borrowed-buffer axis call. Above this the retained path falls back to the
 * complete cold path. Initial candidate per §50; final values come from
 * realistic traces at T15.
 */
export const MAX_DIRECT_AXIS_REFS = 1_024;

/** PERF-12 T10 (§30/§36): reusable flat-grid construction scratch cap. */
export const MAX_DIRECT_GRID_WORDS = 65_536;

/**
 * PERF-12 T11 (§30/§37/§41): retained payload scratch caps. Text spans and
 * diff payloads ride one reusable byte tier; diff framing words reuse the
 * same u32 tier sizing as Grid. Above a cap the retained path refuses and
 * routes the complete cold path (§50).
 */
export const MAX_DIRECT_TEXT_BYTES = 65_536;
export const MAX_DIRECT_DIFF_WORDS = 65_536;
export const MAX_DIRECT_DIFF_BYTES = 65_536;
