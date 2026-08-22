/**
 * PERF-12 T7 gate benchmark (§102.1 smoke profile): one representative
 * SHARED_PATH retained case must beat or tie direct_7v2 TOTAL time before
 * broader generation proceeds.
 *
 * Shape: a stable 200-node subtree S plus a rebuilt changed branch
 * (root -> branch -> leaf, three fresh semantic nodes per step).
 *
 *   retained_dag_ffi  production construction + RetainedRootBoundary.install:
 *                     children-first generated FFI materialization of the
 *                     three new nodes, identity cutoff at S, one hostRenderRef.
 *   direct_7v2        identical construction + NativeTuiHost.render through
 *                     the N-API Direct decoder (NodeId-first cache cutoff).
 *
 * Both arms pay identical JS construction and identical host repaint; the
 * differential is changed-frontier transport: monomorphic FFI constructors
 * versus the N-API property walk. Total wall time decides (§90/§119).
 */

import { native } from "../src/native.ts";
import { nodeForBridge, View } from "../src/tui/values/view.ts";
import { nativeViewAbiSession } from "../src/tui/native_view_abi.ts";
import { RetainedRootBoundary } from "../src/tui/retained_dag.ts";
import { mkdirSync } from "node:fs";

const STABLE_NODES = 200;
const OPS_PER_BLOCK = 20;
const WARMUP_BLOCKS = 60;
const MAX_BLOCKS = 400;
const TARGET_CI_HALF_WIDTH_RATIO = 0.03;

type Host = {
  render(view: object): void;
  tuiViewAbiHostPointer(): number;
  dispose(): void;
};

const Host = native.NativeTuiHost as unknown as
  | (new (width: number, height: number, headless: boolean) => Host)
  | undefined;

function buildColumnTree(total: number): View {
  return View.vertical((builder) => {
    for (let index = 0; index < total - 1; index += 1) builder.child(View.spacer(1));
  });
}

/** One changed generation: three fresh semantic nodes sharing stable S. */
function buildNextGeneration(step: number, stable: View): View {
  const leafRows = (step % 4) + 1;
  return View.vertical([View.vertical([View.spacer(leafRows)]), stable]);
}

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
  if (Host === undefined) throw new Error("PERF-12 T7 benchmark requires the staged NativeTuiHost artifact");
  const session = nativeViewAbiSession();
  if (session === undefined) throw new Error("PERF-12 T7 benchmark requires the native View ABI session");
  const maintain = (
    native as { tuiViewAbiMaintain?: () => unknown }
  ).tuiViewAbiMaintain;

  // Shared stable subtree identity for BOTH arms: rendered once via Direct,
  // its frozen BridgeViewNodes are reused by every constructed generation.
  const stable = buildColumnTree(STABLE_NODES);

  // ---- retained_dag_ffi arm -------------------------------------------------
  const retainedHost = new Host(60, 16, true);
  // Seed generation rendered via Direct, then adopted by the boundary: the
  // stable subtree's frozen BridgeViewNodes carry the shared identity.
  const seed = View.vertical([View.spacer(1), stable]);
  retainedHost.render(nodeForBridge(seed));
  const boundary = new RetainedRootBoundary(session, () => retainedHost.tuiViewAbiHostPointer() as never);
  if (!boundary.adopt(seed)) throw new Error("retained arm adoption failed");

  // ---- direct_7v2 arm -------------------------------------------------------
  const directHost = new Host(60, 16, true);
  directHost.render(nodeForBridge(View.vertical([View.spacer(1), stable])));

  const runRetainedBlock = (ops: number): void => {
    for (let step = 0; step < ops; step += 1) {
      const next = buildNextGeneration(step, stable);
      if (boundary.install(next) === undefined) throw new Error("retained install fell back");
    }
  };
  const runDirectBlock = (ops: number): void => {
    for (let step = 0; step < ops; step += 1) {
      const next = buildNextGeneration(step, stable);
      directHost.render(nodeForBridge(next));
    }
  };

  // Warmup both arms thoroughly (also populates the Direct decoder cache).
  for (let block = 0; block < WARMUP_BLOCKS; block += 1) {
    runDirectBlock(OPS_PER_BLOCK);
    runRetainedBlock(OPS_PER_BLOCK);
  }

  const sampleBlocks = (
    label: string,
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

  // Alternate arm order between rounds to cancel drift (§100).
  const directFirst = sampleBlocks("direct_7v2", runDirectBlock);
  const retainedFirst = sampleBlocks("retained_dag_ffi", runRetainedBlock);

  const records: Record<string, unknown>[] = [];
  for (const [candidate, stats] of [
    ["direct_7v2", directFirst],
    ["retained_dag_ffi", retainedFirst],
  ] as const) {
    records.push({
      record_kind: "t7_shared_path_case",
      profile: "smoke",
      candidate,
      workload: "shared_path_changed_branch",
      mode: "SHARED_PATH",
      size: STABLE_NODES,
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

  const ratio = retainedFirst.medianNs / directFirst.medianNs;
  const commandText = (command: string[]): string =>
    new TextDecoder().decode(Bun.spawnSync(command).stdout).trim() || "unknown";
  records.push({
    record_kind: "t7_shared_path_summary",
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
    direct_7v2_median_ns: Math.round(directFirst.medianNs),
    retained_median_ns: Math.round(retainedFirst.medianNs),
    retained_over_direct_median_ratio: Number(ratio.toFixed(4)),
    gate: ratio <= 1 ? "beats" : ratio <= 1.02 ? "ties" : "loses",
  });

  console.log(`direct_7v2       median ${Math.round(directFirst.medianNs)}ns/op  p95 ${Math.round(directFirst.p95Ns)}ns`);
  console.log(`retained_dag_ffi median ${Math.round(retainedFirst.medianNs)}ns/op  p95 ${Math.round(retainedFirst.p95Ns)}ns`);
  console.log(`ratio retained/direct: ${ratio.toFixed(4)} (${ratio <= 1 ? "beats" : ratio <= 1.02 ? "ties" : "loses"})`);

  boundary.close();
  retainedHost.dispose();
  directHost.dispose();

  const outPath = Bun.env.PERF12_T7_OUT ?? "packages/iyon-runtime/bench/PERF-12-t7-shared-path.jsonl";
  mkdirSync("packages/iyon-runtime/bench", { recursive: true });
  await Bun.write(outPath, records.map((record) => JSON.stringify(record)).join("\n") + "\n");
  console.log(`wrote ${outPath}`);
}

await main();
