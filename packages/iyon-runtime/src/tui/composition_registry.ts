/**
 * PERF-12 T13.1 — process-global composition module registry (§9).
 *
 * @internal Framework-private. The source transform injects one
 * `registerCompositionModule(siteCount)` call per transformed module at
 * initialization and receives a dense process-local module id; site ids are
 * dense within the module (AST/source order, §9.3). No stable cross-build ids
 * exist and none are needed: a composition root lives inside one running
 * program image only (§9.4).
 */

const MODULE_SITE_COUNTS: number[] = [];

/**
 * Registers a transformed module with its dense transformed-site count and
 * returns the process-local module id.
 */
export function registerCompositionModule(siteCount: number): number {
  if (!Number.isInteger(siteCount) || siteCount < 0) {
    throw new RangeError(`composition module site count must be a non-negative integer, got ${siteCount}`);
  }
  MODULE_SITE_COUNTS.push(siteCount);
  return MODULE_SITE_COUNTS.length - 1;
}

/** The declared transformed-site count for a registered module id. */
export function compositionModuleSiteCount(moduleId: number): number | undefined {
  return MODULE_SITE_COUNTS[moduleId];
}
