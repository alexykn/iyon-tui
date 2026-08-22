/**
 * PERF-12 T6 §113 exact-identity scaling benchmark (smoke profile, §102.1).
 *
 * For each tree size (20 / 200 / 2,000 / 10,000 nodes):
 *   1. build the identical root through the public API,
 *   2. populate native state once via Direct decode (cold-sidecar gap, §94),
 *   3. adopt the root into a retained boundary (one NodeId promotion),
 *   4. render the exact same root repeatedly through the §20 fast path.
 *
 * Required structural result per render: 0 semantic field reads,
 * 0 children visited, 0 materializer calls, 0 buffer words written,
 * exactly 1 host FFI call. Timing must not scale with descendant count:
 * the host short-circuits an unchanged body ref, so each exact render is
 * one engine-native call plus two identity comparisons.
 *
 * Sampling per §102.1 tiny cases: blocks of 1,000 ops, warmup >= 50 blocks,
 * adaptive measurement until the bootstrap CI half-width of the median drops
 * below 5% or 500 blocks. Fresh process per invocation; output JSONL marked
 * "profile": "smoke" with the §103 provenance fields.
 */

import { native } from "../src/native.ts";
import { nodeForBridge, View } from "../src/tui/values/view.ts";
import { nativeViewAbiSession } from "../src/tui/native_view_abi.ts";
import {
  renderExactRoot,
  resetRetainedIdentityCounters,
  retainedIdentityCounterSnapshot,
  RetainedRootBoundary,
} from "../src/tui/retained_dag.ts";
import { mkdirSync } from "node:fs";

const SIZES = [20, 200, 2_000, 10_000];
const OPS_PER_BLOCK = 1_000;
const WARMUP_BLOCKS = 50;
const MAX_BLOCKS = 500;
const TARGET_CI_HALF_WIDTH_RATIO = 0.05;

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
    for (let index = 0; index < total - 1; index += 1) builder.child(View.text("x"));
  });
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

/** Percentile bootstrap CI of the median from block samples. */
function bootstrapCi95(values: number[], resamples = 1_000): [number, number] {
  const medians: number[] = [];
  for (let index = 0; index < resamples; index += 1) {
    const sample = Array.from({ length: values.length }, () => values[(Math.random() * values.length) | 0]!);
    medians.push(median(sample));
  }
  medians.sort((a, b) => a - b);
  return [medians[Math.floor(resamples * 0.025)]!, medians[Math.floor(resamples * 0.975)]!];
}

function runBlock(
  session: NonNullable<ReturnType<typeof nativeViewAbiSession>>,
  host: Host,
  view: View,
  ops: number,
): void {
  const hostPointer = host.tuiViewAbiHostPointer();
  for (let index = 0; index < ops; index += 1) {
    const result = renderExactRoot(session, hostPointer as never, view);
    if (result.status !== "ok") throw new Error("exact render failed during timed block");
  }
}

async function main() {
  if (Host === undefined) throw new Error("PERF-12 T6 benchmark requires the staged NativeTuiHost artifact");
  const session = nativeViewAbiSession();
  if (session === undefined) throw new Error("PERF-12 T6 benchmark requires the native View ABI session");

  const records: Record<string, unknown>[] = [];
  const mediansBySize: { size: number; medianNs: number }[] = [];

  for (const size of SIZES) {
    const view = buildColumnTree(size);
    const host = new Host(80, 8, true);
    try {
      // Cold population via Direct decode; no JS hints exist yet (§94 shape).
      host.render(nodeForBridge(view));
      const boundary = new RetainedRootBoundary(session, () => host.tuiViewAbiHostPointer() as never);
      if (!boundary.adopt(view)) throw new Error(`adopt failed at size ${size}`);

      for (let block = 0; block < WARMUP_BLOCKS; block += 1) runBlock(session, host, view, OPS_PER_BLOCK);

      resetRetainedIdentityCounters();
      const blockMedians: number[] = [];
      let countersAfter = retainedIdentityCounterSnapshot();
      for (let block = 0; block < MAX_BLOCKS; block += 1) {
        Bun.gc(true);
        const start = Bun.nanoseconds();
        runBlock(session, host, view, OPS_PER_BLOCK);
        const nsPerOp = Number(Bun.nanoseconds() - start) / OPS_PER_BLOCK;
        blockMedians.push(nsPerOp);
        countersAfter = retainedIdentityCounterSnapshot();
        if (blockMedians.length >= 100) {
          const [lo, hi] = bootstrapCi95(blockMedians);
          const mid = median(blockMedians);
          if ((hi - lo) / 2 <= TARGET_CI_HALF_WIDTH_RATIO * mid) break;
        }
      }

      const [ciLo, ciHi] = bootstrapCi95(blockMedians);
      const medianNs = median(blockMedians);
      mediansBySize.push({ size, medianNs });

      // Structural gate: across every measured op exactly one host FFI call
      // per render and zero semantic/buffer work anywhere on the JS side.
      const expectedRenders = blockMedians.length * OPS_PER_BLOCK;
      const structuralOk =
        countersAfter.host_mutations === expectedRenders
        && countersAfter.bridge_hint_misses === 0
        && countersAfter.bridge_semantic_nodes_inspected === 0
        && countersAfter.bridge_children_visited === 0
        && countersAfter.direct_materializer_calls === 0
        && countersAfter.node_id_ref_promotion_attempts === 0
        && countersAfter.ref_words_written === 0
        && countersAfter.byte_payload_bytes === 0
        && countersAfter.stale_ref_retries === 0
        && countersAfter.cold_fallbacks === 0;
      if (!structuralOk) {
        throw new Error(`structural gate failed at size ${size}: ${JSON.stringify(countersAfter)}`);
      }

      records.push({
        record_kind: "t6_exact_identity_case",
        profile: "smoke",
        candidate: "retained_dag_ffi",
        workload: "exact_identity_scaling",
        mode: "IDENTICAL_IDENTITY",
        size,
        nodes: size,
        ops_per_block: OPS_PER_BLOCK,
        blocks: blockMedians.length,
        samples_ns: blockMedians.map((value) => Math.round(value)),
        median_ns: Math.round(medianNs),
        p95_ns: Math.round(percentile(blockMedians, 0.95)),
        p99_ns: Math.round(percentile(blockMedians, 0.99)),
        median_ci95_ns: [Math.round(ciLo), Math.round(ciHi)],
        structural_counters: countersAfter,
        expected_host_ffl_calls_per_render: 1,
      });
      console.log(
        `size ${String(size).padStart(6)}: median ${Math.round(medianNs)}ns/op `
        + `p95 ${Math.round(percentile(blockMedians, 0.95))}ns `
        + `blocks ${blockMedians.length} renders ${expectedRenders}`,
      );
      boundary.close();
    } finally {
      host.dispose();
    }
  }

  const flat10kOver20 = mediansBySize[3]!.medianNs / mediansBySize[0]!.medianNs;
  const commandText = (command: string[]): string =>
    new TextDecoder().decode(Bun.spawnSync(command).stdout).trim() || "unknown";
  records.push({
    record_kind: "t6_exact_identity_summary",
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
    median_ratio_10k_over_20: Number(flat10kOver20.toFixed(3)),
  });
  console.log(`\nflatness ratio (10k / 20 median): ${flat10kOver20.toFixed(3)}`);

  const outPath = Bun.env.PERF12_T6_OUT ?? "packages/iyon-runtime/bench/PERF-12-t6-exact-identity.jsonl";
  mkdirSync("packages/iyon-runtime/bench", { recursive: true });
  await Bun.write(outPath, records.map((record) => JSON.stringify(record)).join("\n") + "\n");
  console.log(`wrote ${outPath}`);
}

await main();
