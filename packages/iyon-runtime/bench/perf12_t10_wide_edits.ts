/**
 * PERF-12 T10 §96 smoke benchmark: retained wide edits.
 *
 * The retained arm performs one PersistentSeq-backed semantic edit and one
 * native retained edit per operation; no old child sequence crosses FFI.
 * The Direct arm forces the lazy flat children accessor and decodes the full
 * changed axis, providing the complete prior-path comparison. Replace is
 * measured at 2k/10k/100k; insert/remove/splice-four are measured at 2k.
 */

import { native } from "../src/native.ts";
import { nodeForBridge, View } from "../src/tui/values/view.ts";
import { nativeViewAbiSession } from "../src/tui/native_view_abi.ts";
import { RetainedRootBoundary, resetRetainedIdentityCounters, retainedIdentityCounterSnapshot } from "../src/tui/retained_dag.ts";
import { persistentSeqCounters, resetPersistentSeqCounters } from "../src/tui/persistent_seq.ts";
import { mkdirSync } from "node:fs";

type Host = { render(view: object): void; tuiViewAbiHostPointer(): number; dispose(): void };
const Host = native.NativeTuiHost as unknown as (new (width: number, height: number, headless: boolean) => Host) | undefined;
const OPS = 10;
const WARMUP = 20;
const MEASURED = 30;

function wideColumn(width: number): View {
  return View.vertical(Array.from({ length: width }, (_, index) => View.spacer((index % 3) + 1)));
}

function median(values: number[]): number {
  const sorted = [...values].sort((a, b) => a - b);
  const middle = sorted.length >> 1;
  return sorted.length % 2 === 0 ? (sorted[middle - 1]! + sorted[middle]!) / 2 : sorted[middle]!;
}

function percentile(values: number[], fraction: number): number {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * fraction))]!;
}

function bootstrapMedianCi95(values: number[], resamples = 1_000): [number, number] {
  const medians: number[] = [];
  for (let sampleIndex = 0; sampleIndex < resamples; sampleIndex += 1) {
    const sample = Array.from({ length: values.length }, () => values[(Math.random() * values.length) | 0]!);
    medians.push(median(sample));
  }
  medians.sort((left, right) => left - right);
  return [medians[Math.floor(resamples * 0.025)]!, medians[Math.floor(resamples * 0.975)]!];
}

function runCase(
  candidate: "retained_dag_ffi" | "direct_7v2",
  mode: "axis_set" | "axis_insert" | "axis_remove" | "axis_splice4",
  width: number,
  session: ReturnType<typeof nativeViewAbiSession>,
): { samples: number[]; counters: ReturnType<typeof retainedIdentityCounterSnapshot>; seq: typeof persistentSeqCounters } {
  if (Host === undefined || session === undefined) throw new Error("T10 benchmark requires the staged native artifact");
  const host = new Host(80, 24, true);
  const base = wideColumn(width);
  host.render(nodeForBridge(base));
  const boundary = candidate === "retained_dag_ffi"
    ? new RetainedRootBoundary(session, () => host.tuiViewAbiHostPointer() as never)
    : undefined;
  if (boundary !== undefined && !boundary.adopt(base)) throw new Error("T10 adoption failed");

  const next = (current: View, step: number): View => {
    const index = width >> 1;
    if (mode === "axis_set") return View.axisSetChildForTransport(current, index, View.spacer((step % 3) + 1));
    if (mode === "axis_insert") return View.axisSpliceForTransport(current, index, 0, [{ view: View.spacer(3) }]);
    if (mode === "axis_remove") return View.axisSpliceForTransport(current, index, 1, []);
    return View.axisSpliceForTransport(current, index, 4, [
      { view: View.spacer(2) },
      { view: View.spacer(3) },
      { view: View.spacer(4) },
      { view: View.spacer(5) },
    ]);
  };

  let current = base;
  const run = (): void => {
    for (let step = 0; step < OPS; step += 1) {
      current = next(current, step);
      if (candidate === "retained_dag_ffi") {
        if (boundary!.install(current) === undefined) throw new Error(`T10 retained ${mode}@${width} fell back`);
      } else {
        host.render(nodeForBridge(current));
      }
    }
  };
  for (let round = 0; round < WARMUP; round += 1) run();
  resetRetainedIdentityCounters();
  resetPersistentSeqCounters();
  const samples: number[] = [];
  for (let round = 0; round < MEASURED; round += 1) {
    const started = Bun.nanoseconds();
    run();
    samples.push(Number(Bun.nanoseconds() - started) / OPS);
  }
  const counters = retainedIdentityCounterSnapshot();
  const seq = { ...persistentSeqCounters };
  boundary?.close();
  host.dispose();
  return { samples, counters, seq };
}

function commandText(command: string[]): string {
  return new TextDecoder().decode(Bun.spawnSync(command).stdout).trim() || "unknown";
}

async function main(): Promise<void> {
  const session = nativeViewAbiSession();
  if (Host === undefined || session === undefined) throw new Error("T10 benchmark requires the staged NativeTuiHost artifact");
  const records: Record<string, unknown>[] = [];
  const cases: readonly ["axis_set" | "axis_insert" | "axis_remove" | "axis_splice4", number][] = [
    ["axis_set", 32], ["axis_set", 256], ["axis_set", 2_000], ["axis_set", 10_000], ["axis_set", 100_000],
    ["axis_insert", 2_000], ["axis_remove", 2_000], ["axis_splice4", 2_000],
  ];
  for (const [mode, width] of cases) {
    const arms: Partial<Record<"retained_dag_ffi" | "direct_7v2", ReturnType<typeof runCase>>> = {};
    for (const candidate of ["direct_7v2", "retained_dag_ffi"] as const) {
      // The 100k Direct arm is intentionally included: it records the cost
      // of the complete fallback at the exact §96 width, not a shortcut.
      arms[candidate] = runCase(candidate, mode, width, session);
      const result = arms[candidate]!;
      records.push({
        record_kind: "t10_wide_case",
        profile: "smoke",
        candidate,
        workload: mode,
        mode: mode.toUpperCase(),
        size: width,
        warmup_ops: WARMUP * OPS,
        measured_ops: MEASURED * OPS,
        median_ns: Math.round(median(result.samples)),
        p95_ns: Math.round(percentile(result.samples, 0.95)),
        p99_ns: Math.round(percentile(result.samples, 0.99)),
        median_ci95_ns: bootstrapMedianCi95(result.samples).map((value) => Math.round(value)),
        samples_ns: result.samples.map((value) => Math.round(value)),
        persistent_seq_branches_cloned: result.seq.branches_cloned,
        persistent_seq_nodes_cloned: result.seq.nodes_cloned,
        persistent_seq_items_iterated: result.seq.items_iterated,
        derivation_fast_path_calls: result.counters.derivation_fast_path_calls,
        direct_materializer_calls: result.counters.direct_materializer_calls,
        bridge_children_visited: result.counters.bridge_children_visited,
        ref_words_written: result.counters.ref_words_written,
      });
    }
    const direct = arms.direct_7v2!;
    const retained = arms.retained_dag_ffi!;
    records.push({
      record_kind: "t10_wide_summary",
      profile: "smoke",
      workload: mode,
      size: width,
      direct_7v2_median_ns: Math.round(median(direct.samples)),
      retained_median_ns: Math.round(median(retained.samples)),
      retained_over_direct_median_ratio: Number((median(retained.samples) / median(direct.samples)).toFixed(4)),
      retained_structural: {
        derivation_fast_path_calls: retained.counters.derivation_fast_path_calls,
        direct_materializer_calls: retained.counters.direct_materializer_calls,
        bridge_children_visited: retained.counters.bridge_children_visited,
        ref_words_written: retained.counters.ref_words_written,
      },
    });
  }
  records.push({
    record_kind: "t10_wide_provenance",
    profile: "smoke",
    benchmark_version: "PERF-12",
    git_sha: commandText(["git", "rev-parse", "HEAD"]),
    bun_version: Bun.version,
    bun_revision: commandText(["bun", "--revision"]),
    rustc_version: commandText(["rustc", "--version"]),
    target: commandText(["rustc", "-vV"]).split("\n").find((line) => line.startsWith("host:"))?.slice(6),
    addon_sha256: commandText(["shasum", "-a", "256", "packages/iyon-runtime/native/iyon-native.node"]).split(" ")[0],
    macos_version: commandText(["sw_vers", "-productVersion"]),
  });
  mkdirSync("packages/iyon-runtime/bench", { recursive: true });
  const output = Bun.env.PERF12_T10_OUT ?? "packages/iyon-runtime/bench/PERF-12-t10-wide-edits.jsonl";
  await Bun.write(output, records.map((record) => JSON.stringify(record)).join("\n") + "\n");
  for (const record of records.filter((value) => value.record_kind === "t10_wide_summary")) {
    console.log(`${record.workload}@${record.size}: retained/direct=${record.retained_over_direct_median_ratio}`);
  }
  console.log(`wrote ${output}`);
}

await main();
