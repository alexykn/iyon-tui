/**
 * PERF-12 T9 gate benchmark (§102.1 smoke profile): TEXT_METADATA_PATCH.
 *
 * A wrap/align-only text change inside a ~200-node tree must transport base
 * NativeRef + NodeId + scalars on the retained arm (derivation fast path,
 * §27/§38) — never the text payload — and beat or tie direct_7v2 TOTAL time
 * (construction + transport + host repaint).
 *
 *   retained_dag_ffi  production construction + RetainedRootBoundary.install:
 *                     the wrap-changed text rides view_text_layout_patch_root
 *                     (base ref + new NodeId + two scalar codes), the fresh
 *                     spacer leaf rides a generated constructor, identity
 *                     cutoff everywhere else, one hostRenderRef.
 *   direct_7v2        identical construction + NativeTuiHost.render through
 *                     the N-API Direct decoder.
 */

import { native } from "../src/native.ts";
import { nodeForBridge, View } from "../src/tui/values/view.ts";
import { nativeViewAbiSession } from "../src/tui/native_view_abi.ts";
import { RetainedRootBoundary, retainedIdentityCounterSnapshot } from "../src/tui/retained_dag.ts";
import { mkdirSync } from "node:fs";

const STABLE_NODES = 200;
const OPS_PER_BLOCK = 20;
const WARMUP_BLOCKS = 60;
const MAX_BLOCKS = 400;
const TARGET_CI_HALF_WIDTH_RATIO = 0.03;

type Host = {
  render(view: object): void;
  screenRows(): string[];
  tuiViewAbiHostPointer(): number;
  dispose(): void;
};

const Host = native.NativeTuiHost as unknown as
  | (new (width: number, height: number, headless: boolean) => Host)
  | undefined;

const WRAP_CYCLE = ["wordThenGrapheme", "grapheme"] as const;

function median(values: number[]): number {
  const sorted = [...values].sort((a, b) => a - b);
  const mid = sorted.length >> 1;
  return sorted.length % 2 === 0 ? (sorted[mid - 1]! + sorted[mid]!) / 2 : sorted[mid]!;
}

function percentile(values: number[], fraction: number): number {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * fraction))]!;
}

function bootstrapCi95(values: number[], resamples = 1_000): [number, number] {
  const medians: number[] = [];
  for (let index = 0; index < resamples; index += 1) {
    const sample = Array.from({ length: values.length }, () => values[(Math.random() * values.length) | 0]!);
    medians.push(median(sample));
  }
  medians.sort((a, b) => a - b);
  return [medians[Math.floor(resamples * 0.025)]!, medians[Math.floor(resamples * 0.975)]!];
}

async function main() {
  if (Host === undefined) throw new Error("PERF-12 T9 benchmark requires the staged NativeTuiHost artifact");
  const session = nativeViewAbiSession();
  if (session === undefined) throw new Error("PERF-12 T9 benchmark requires the native View ABI session");
  const maintain = (
    native as { tuiViewAbiMaintain?: () => unknown }
  ).tuiViewAbiMaintain;

  // The stable spine plus the long-lived semantic text node whose layout
  // metadata changes once per generation (§93 TEXT_METADATA_PATCH). Each
  // generation derives from the PREVIOUS generation's text node, so the
  // derivation base always lives inside the previously leased root (§18).
  const stable = View.vertical((builder) => {
    for (let index = 0; index < STABLE_NODES - 2; index += 1) builder.child(View.spacer(1));
  });
  const text = View.text("The quick brown fox jumps over the lazy dog");

  /** Per-arm chained generation builder: wrap alternates each generation. */
  const makeChainer = () => {
    let currentText = text;
    let step = 0;
    return (): View => {
      step += 1;
      currentText = currentText.wrap(WRAP_CYCLE[step % 2]!);
      return View.vertical([View.spacer((step % 3) + 1), stable, currentText]);
    };
  };

  // ---- retained_dag_ffi arm -------------------------------------------------
  const retainedHost = new Host(80, 24, true);
  const seed = View.vertical([View.spacer(1), stable, text]);
  retainedHost.render(nodeForBridge(seed));
  const boundary = new RetainedRootBoundary(session, () => retainedHost.tuiViewAbiHostPointer() as never);
  if (!boundary.adopt(seed)) throw new Error("retained arm adoption failed");
  const nextRetainedGeneration = makeChainer();
  if (boundary.install(nextRetainedGeneration()) === undefined) {
    throw new Error("retained derivation install fell back during setup");
  }

  // ---- direct_7v2 arm -------------------------------------------------------
  const directHost = new Host(80, 24, true);
  directHost.render(nodeForBridge(View.vertical([View.spacer(1), stable, text])));
  const nextDirectGeneration = makeChainer();

  const runRetainedBlock = (ops: number): void => {
    for (let index = 0; index < ops; index += 1) {
      if (boundary.install(nextRetainedGeneration()) === undefined) {
        throw new Error("retained derivation install fell back");
      }
    }
  };
  const runDirectBlock = (ops: number): void => {
    for (let index = 0; index < ops; index += 1) directHost.render(nodeForBridge(nextDirectGeneration()));
  };

  for (let block = 0; block < WARMUP_BLOCKS; block += 1) {
    runDirectBlock(OPS_PER_BLOCK);
    runRetainedBlock(OPS_PER_BLOCK);
  }

  const sampleBlocks = (
    run: (ops: number) => void,
  ): { samples: number[]; medianNs: number; p95Ns: number; p99Ns: number; ci95: [number, number]; blocks: number } => {
    const samples: number[] = [];
    let ci: [number, number] = [0, 0];
    for (let block = 0; block < MAX_BLOCKS; block += 1) {
      Bun.gc(true);
      maintain?.call(native);
      const start = Bun.nanoseconds();
      run(OPS_PER_BLOCK);
      samples.push(Number(Bun.nanoseconds() - start) / OPS_PER_BLOCK);
      if (samples.length >= 80) {
        ci = bootstrapCi95(samples);
        if ((ci[1] - ci[0]) / 2 <= TARGET_CI_HALF_WIDTH_RATIO * median(samples)) break;
      }
    }
    return {
      samples,
      medianNs: median(samples),
      p95Ns: percentile(samples, 0.95),
      p99Ns: percentile(samples, 0.99),
      ci95: bootstrapCi95(samples),
      blocks: samples.length,
    };
  };

  const beforeCounters = retainedIdentityCounterSnapshot();
  const directStats = sampleBlocks(runDirectBlock);
  const retainedStats = sampleBlocks(runRetainedBlock);
  const afterCounters = retainedIdentityCounterSnapshot();

  // Structural proof for the retained arm's measured window: every changed
  // generation rode the derivation lane with zero payload bytes transported.
  const derivations = afterCounters.derivation_fast_path_calls - beforeCounters.derivation_fast_path_calls;
  const fallbacks = afterCounters.cold_fallbacks - beforeCounters.cold_fallbacks;
  const payloadBytes = afterCounters.byte_payload_bytes - beforeCounters.byte_payload_bytes;
  if (fallbacks !== 0 || payloadBytes !== 0) {
    throw new Error(`structural violation during measurement: fallbacks=${fallbacks} payloadBytes=${payloadBytes}`);
  }
  if (derivations < retainedStats.blocks * OPS_PER_BLOCK) {
    throw new Error(`expected at least one derivation per retained op, got ${derivations}`);
  }

  const records: Record<string, unknown>[] = [];
  for (const [candidate, stats] of [
    ["direct_7v2", directStats],
    ["retained_dag_ffi", retainedStats],
  ] as const) {
    records.push({
      record_kind: "t9_text_metadata_patch_case",
      profile: "smoke",
      candidate,
      workload: "text_metadata_patch",
      mode: "TEXT_METADATA_PATCH",
      size: STABLE_NODES + 2,
      stable_nodes: STABLE_NODES,
      changed_frontier_nodes: 3,
      ops_per_block: OPS_PER_BLOCK,
      blocks: stats.blocks,
      samples_ns: stats.samples.map((value) => Math.round(value)),
      median_ns: Math.round(stats.medianNs),
      p95_ns: Math.round(stats.p95Ns),
      p99_ns: Math.round(stats.p99Ns),
      median_ci95_ns: [Math.round(stats.ci95[0]), Math.round(stats.ci95[1])],
    });
  }

  const ratio = retainedStats.medianNs / directStats.medianNs;
  const commandText = (command: string[]): string =>
    new TextDecoder().decode(Bun.spawnSync(command).stdout).trim() || "unknown";
  records.push({
    record_kind: "t9_text_metadata_patch_summary",
    profile: "smoke",
    git_sha: commandText(["git", "rev-parse", "HEAD"]),
    bun_version: Bun.version,
    bun_revision: commandText(["bun", "--revision"]),
    rustc_version: commandText(["rustc", "--version"]),
    target: commandText(["rustc", "-vV"]).split("\n").find((line) => line.startsWith("host:"))?.slice(6),
    macos_version: commandText(["sw_vers", "-productVersion"]),
    cpu_model: commandText(["sysctl", "-n", "machdep.cpu.brand_string"]),
    addon_sha256: commandText(["shasum", "-a", "256", "packages/iyon-runtime/native/iyon-native.node"]).split(" ")[0],
    warmup_blocks: WARMUP_BLOCKS,
    max_blocks: MAX_BLOCKS,
    ops_per_block: OPS_PER_BLOCK,
    structural_counters: {
      derivation_fast_path_calls: derivations,
      cold_fallbacks: fallbacks,
      byte_payload_bytes: payloadBytes,
    },
    direct_7v2_median_ns: Math.round(directStats.medianNs),
    retained_median_ns: Math.round(retainedStats.medianNs),
    retained_over_direct_median_ratio: Number(ratio.toFixed(4)),
    gate: ratio <= 1 ? "beats" : ratio <= 1.02 ? "ties" : "loses",
  });

  console.log(`direct_7v2       median ${Math.round(directStats.medianNs)}ns/op  p95 ${Math.round(directStats.p95Ns)}ns`);
  console.log(`retained_dag_ffi median ${Math.round(retainedStats.medianNs)}ns/op  p95 ${Math.round(retainedStats.p95Ns)}ns`);
  console.log(`ratio retained/direct: ${ratio.toFixed(4)} (${ratio <= 1 ? "beats" : ratio <= 1.02 ? "ties" : "loses"})`);
  console.log(`derivations=${derivations} fallbacks=${fallbacks} payloadBytes=${payloadBytes}`);

  boundary.close();
  retainedHost.dispose();
  directHost.dispose();

  const outPath = Bun.env.PERF12_T9_OUT ?? "packages/iyon-runtime/bench/PERF-12-t9-text-metadata-patch.jsonl";
  mkdirSync("packages/iyon-runtime/bench", { recursive: true });
  await Bun.write(outPath, records.map((record) => JSON.stringify(record)).join("\n") + "\n");
  console.log(`wrote ${outPath}`);
}

await main();
