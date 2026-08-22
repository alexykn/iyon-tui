/**
 * PERF-12.0 memory attribution child (PERF-12 handoff §57–§58).
 *
 * Runs ONE benchmark block in an isolated process against the frozen
 * PERF-11v4 native artifact and harness fixtures, recording the §57 counter
 * set at four phases:
 *
 *   pre            before any fixture exists
 *   fixtures       after the retained base case is constructed
 *   post_workload  after the operation loop (§57 snapshot #1)
 *   post_cleanup   after release/close all roots -> Bun.gc(true) ->
 *                  native maintenance/full weak sweep -> Bun.gc(true)
 *
 * The parent (`perf12_memory_attribution.ts`) spawns one child per block and
 * writes PERF-12-memory-attribution.jsonl. This tool makes no GC claims on
 * its own; classification into §58 buckets happens in the baseline record
 * from these raw numbers.
 */

import { native } from "../src/native.ts";
import { appendFileSync as appendSync } from "node:fs";
import { nodeForDirectBridge, View } from "../src/tui/values/view.ts";
import { History } from "../src/tui/history.ts";
import { TextStream } from "../src/tui/stream.ts";
import { buildTracePair, prepareComparisonCase, setComparisonComponentId, type ComparisonMode, type ComparisonWorkload } from "./perf11v4_fixtures.ts";

interface BlockConfig {
  readonly label: string;
  readonly kind: "retained" | "first_use" | "trace";
  readonly workload?: ComparisonWorkload;
  readonly size?: number;
  readonly mode?: ComparisonMode;
  readonly ops: number;
}

interface NativeDiagnostics {
  readonly semantic_cache_entries: number;
  readonly native_ref_slots: number;
  readonly leased_slots: number;
  readonly path_nodes: number;
  readonly builders: number;
  readonly edit_transactions: number;
  readonly style_atoms: number;
  readonly styles: number;
  readonly stale_removals: number;
  readonly release_batches: number;
  readonly released_refs: number;
  readonly live_weak_upgrades: number;
  readonly generation: number;
  readonly alive: boolean;
}

interface PhaseSnapshot {
  readonly rss_bytes: number;
  readonly peak_rss_bytes: number;
  readonly js_heap_used_bytes: number;
  readonly js_heap_total_bytes: number;
  readonly js_external_bytes: number | null;
  readonly js_array_buffers_bytes: number | null;
  readonly native: NativeDiagnostics;
  readonly extra?: Record<string, unknown>;
}

interface NativeHost {
  render(view: object): void;
  screenRows(): string[];
  dispose(): void;
  advanceTime?(milliseconds: number): void;
  history(): object;
  setHistory(history: object): void;
  createViewSlot(initial: object): { componentId(): number | null; setView?(view: object): void; dispose?(): void };
}

function blockConfig(): BlockConfig {
  const raw = Bun.env.PERF12_BLOCK_JSON;
  if (raw === undefined || raw === "") throw new Error("PERF12_BLOCK_JSON is required");
  const parsed = JSON.parse(raw) as BlockConfig;
  if (!Number.isSafeInteger(parsed.ops) || parsed.ops <= 0) throw new Error("block ops must be a positive integer");
  return parsed;
}

function commandText(command: string[]): string {
  return new TextDecoder().decode(Bun.spawnSync(command).stdout).trim() || "unknown";
}

function sha256(path: string): string {
  return commandText(["shasum", "-a", "256", path]).split(/\s+/)[0] ?? "unknown";
}

function now(): number {
  return Bun.nanoseconds();
}

function nativeDiagnostics(pruneExpired: boolean): NativeDiagnostics {
  const bootstrap = native.tuiViewAbiBootstrap?.(pruneExpired);
  if (bootstrap === undefined) throw new Error("native addon does not expose tuiViewAbiBootstrap");
  return bootstrap.diagnostics as unknown as NativeDiagnostics;
}

function snapshot(extra?: Record<string, unknown>): PhaseSnapshot {
  const usage = process.memoryUsage();
  return {
    rss_bytes: usage.rss,
    peak_rss_bytes: (process.resourceUsage?.().maxRSS ?? 0) * 1024,
    js_heap_used_bytes: usage.heapUsed,
    js_heap_total_bytes: usage.heapTotal,
    js_external_bytes: usage.external ?? null,
    js_array_buffers_bytes: usage.arrayBuffers ?? null,
    native: nativeDiagnostics(false),
    ...(extra === undefined ? {} : { extra }),
  };
}

/** Counts unique objects reachable from a bridge node graph (nodes, spans, styles, payloads). */
function countLiveBridgeObjects(root: object): number {
  const seen = new WeakSet<object>();
  let count = 0;
  const visit = (value: unknown, depth: number): void => {
    if (value === null || typeof value !== "object" || depth > 24) return;
    if (seen.has(value as object)) return;
    seen.add(value as object);
    count += 1;
    if (Array.isArray(value)) {
      for (const item of value) visit(item, depth + 1);
      return;
    }
    for (const item of Object.values(value as Record<string, unknown>)) visit(item, depth + 1);
  };
  visit(root, 0);
  return count;
}

function sampleStorageBytes(arrays: readonly number[][]): number {
  // Retained Float64/number array payload is ~8 bytes per element plus small
  // header overhead per array; this is an explicit estimate, not a GC claim.
  return arrays.reduce((sum, array) => sum + array.length * 8 + 128, 0);
}

async function main(): Promise<void> {
  const traceProgress = Bun.env.PERF12_TRACE_PROGRESS === "1";
  const mark = (label: string): void => {
    if (traceProgress) appendSync("/tmp/perf12_child_progress.log", `${label}\n`);
  };
  mark("start");
  const config = blockConfig();
  const pre = snapshot();
  mark("pre snapshot done");

  const host: NativeHost = (() => {
    const Host = native.NativeTuiHost;
    if (Host === undefined) throw new Error("NativeTuiHost is unavailable");
    return new Host(80, 24, true) as unknown as NativeHost;
  })();
  mark("host created");

  const isTrace = config.kind === "trace";
  const prepared = config.kind === "trace"
    ? undefined
    : prepareComparisonCase<View>("current", { workload: config.workload!, size: config.size!, mode: config.mode!, label: config.label });
  const traceHistory = isTrace ? new History() : undefined;
  let traceStream: TextStream | undefined;
  if (traceHistory !== undefined) {
    for (let index = 0; index < 32; index += 1) traceHistory.push(View.text(`history-${index}-stable`));
    traceStream = new TextStream();
    traceHistory.pushStream(traceStream);
    host.setHistory(traceHistory.nativeObject());
    mark("history seeded");
  }
  const componentSlot = isTrace ? host.createViewSlot(nodeForDirectBridge(View.spacer(0))) : undefined;
  if (componentSlot?.componentId() !== null && componentSlot?.componentId() !== undefined) setComparisonComponentId(componentSlot.componentId()!);
  mark("slot created");

  let fixtureRoots: readonly object[] = [];
  if (prepared?.base !== undefined) {
    host.render(nodeForDirectBridge(prepared.base));
    fixtureRoots = [nodeForDirectBridge(prepared.base)];
  }
  const fixturesSnapshot = snapshot({
    fixture_live_bridge_objects: fixtureRoots.reduce((sum, root) => sum + countLiveBridgeObjects(root), 0),
  });

  const totalSamples: number[] = [];
  const constructionSamples: number[] = [];
  let lastRows: readonly string[] = [];
  try {
    for (let index = 0; index < config.ops; index += 1) {
      mark(`op ${index}`);
      const iterationHost = config.kind === "first_use"
        ? new (native.NativeTuiHost as unknown as new(width: number, height: number, headless: boolean) => NativeHost)(80, 24, true)
        : host;
      const started = now();
      const pair = isTrace ? buildTracePair<View>("current", index) : undefined;
      const next = pair?.next ?? prepared!.next(index);
      const constructionNs = now() - started;
      if (!pair?.cold && pair !== undefined) iterationHost.render(nodeForDirectBridge(pair.base));
      const transportStarted = now();
      if (pair?.category === "stream_append") {
        traceStream?.append(`stream-${index} αβ\n`);
      } else if (pair?.category === "history_update") {
        if (traceStream !== undefined) traceHistory?.sealStream(traceStream);
        traceHistory?.push(View.text(`history-insert-${index}`));
        traceStream = new TextStream();
        traceHistory?.pushStream(traceStream);
      } else if (pair?.category === "component_update") {
        componentSlot?.setView?.(nodeForDirectBridge(View.text(`status-${index}`)));
      }
      const nativeStarted = now();
      iterationHost.render(nodeForDirectBridge(next));
      const end = now();
      iterationHost.advanceTime?.(0);
      lastRows = iterationHost.screenRows();
      if (iterationHost !== host) iterationHost.dispose();
      totalSamples.push(constructionNs + (nativeStarted - transportStarted) + (end - nativeStarted));
      constructionSamples.push(constructionNs);
    }
  } finally {
    componentSlot?.dispose?.();
  }

  const postWorkload = snapshot({
    last_screen_rows_valid: Array.isArray(lastRows),
  });
  const samplesBytes = sampleStorageBytes([totalSamples, constructionSamples]);

  // Forced cleanup checkpoint (§57):
  //   release/close all benchmark roots -> Bun.gc(true) ->
  //   native maintenance / full weak sweep -> Bun.gc(true)
  host.dispose();
  let postCleanup = snapshot();
  {
    Bun.gc(true);
    const swept = nativeDiagnostics(true);
    Bun.gc(true);
    postCleanup = snapshot();
    postCleanup = { ...postCleanup, native: swept };
  }

  const result = {
    record_kind: "memory_attribution_block",
    label: config.label,
    kind: config.kind,
    workload: config.workload ?? null,
    size: config.size ?? null,
    mode: config.mode ?? null,
    operations: config.ops,
    git_sha: commandText(["git", "rev-parse", "HEAD"]),
    bun_version: Bun.version,
    bun_revision: commandText(["bun", "--revision"]),
    rustc_version: commandText(["rustc", "--version"]),
    target: commandText(["rustc", "-vV"]).split("\n").find((line) => line.startsWith("host:"))?.split(/\s+/)[1] ?? "unknown",
    macos_version: commandText(["sw_vers", "-productVersion"]),
    cpu_model: commandText(["sysctl", "-n", "machdep.cpu.brand_string"]),
    native_artifact_sha256: sha256(new URL("../native/iyon-native.node", import.meta.url).pathname),
    phases: {
      pre,
      fixtures_loaded: fixturesSnapshot,
      post_workload: postWorkload,
      post_cleanup: postCleanup,
    },
    observations: {
      raw_sample_storage_estimate_bytes: samplesBytes,
      total_median_ns: totalSamples.length > 0 ? [...totalSamples].sort((a, b) => a - b)[Math.floor(totalSamples.length / 2)] : 0,
      path_keys_count: null,
      node_ref_entries: null,
      note_path_keys_node_refs: "not exposed by the current diagnostic ABI; recorded as null for T1 (diagnostic ABI extension is Tranche 3, §89)",
    },
    delta_post_workload_minus_pre: {
      rss_bytes: postWorkload.rss_bytes - pre.rss_bytes,
      js_heap_used_bytes: postWorkload.js_heap_used_bytes - pre.js_heap_used_bytes,
      semantic_cache_entries: postWorkload.native.semantic_cache_entries - pre.native.semantic_cache_entries,
      native_ref_slots: postWorkload.native.native_ref_slots - pre.native.native_ref_slots,
    },
    residual_after_cleanup: {
      rss_bytes: postCleanup.rss_bytes,
      peak_rss_bytes: postCleanup.peak_rss_bytes,
      js_heap_used_bytes: postCleanup.js_heap_used_bytes,
      semantic_cache_entries: postCleanup.native.semantic_cache_entries,
      semantic_cache_live: postCleanup.native.live_weak_upgrades,
      expired_semantic_cache_entries: Math.max(0, postCleanup.native.semantic_cache_entries - postCleanup.native.live_weak_upgrades),
      native_ref_slots: postCleanup.native.native_ref_slots,
      leased_slots: postCleanup.native.leased_slots,
      unleased_native_ref_slots: Math.max(0, postCleanup.native.native_ref_slots - postCleanup.native.leased_slots),
    },
  };
  process.stdout.write(`${JSON.stringify(result)}\n`);
}

await main();
