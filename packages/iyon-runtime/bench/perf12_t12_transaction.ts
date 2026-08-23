/** PERF-12 T12 §43/§44/§45/§47/§118 smoke evidence. */

import { writeFileSync } from "node:fs";
import { native } from "../src/native.ts";
import { nativeViewAbiSession } from "../src/tui/native_view_abi.ts";
import { nodeForBridge, View } from "../src/tui/values/view.ts";
import {
  retainedIdentityCounterSnapshot,
  resetRetainedIdentityCounters,
  RetainedRootBoundary,
} from "../src/tui/retained_dag.ts";

type Host = { render(view: object): void; tuiViewAbiHostPointer(): number; dispose(): void };
const Host = native.NativeTuiHost as unknown as
  | (new (width: number, height: number, headless: boolean) => Host)
  | undefined;
const WARMUP = 20;
const MEASURED = 50;

function median(values: number[]): number {
  const sorted = [...values].sort((a, b) => a - b);
  const middle = sorted.length >> 1;
  return sorted.length % 2 === 0 ? (sorted[middle - 1]! + sorted[middle]!) / 2 : sorted[middle]!;
}

function percentile(values: number[], fraction: number): number {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * fraction))]!;
}

function commandText(command: string[]): string {
  return new TextDecoder().decode(Bun.spawnSync(command).stdout).trim() || "unknown";
}

function runCase(session: ReturnType<typeof nativeViewAbiSession>): {
  samples: number[];
  counters: ReturnType<typeof retainedIdentityCounterSnapshot>;
} {
  if (Host === undefined || session === undefined) throw new Error("T12 benchmark requires the staged native artifact");
  const host = new Host(80, 24, true);
  const initial = View.spacer(1);
  host.render(nodeForBridge(initial));
  const boundary = new RetainedRootBoundary(session, () => host.tuiViewAbiHostPointer() as never);
  if (!boundary.adopt(initial)) throw new Error("T12 boundary adoption failed");
  let current = initial;
  const operation = (): void => {
    const shared = View.spacer(2);
    current = View.horizontal([shared, shared]);
    if (boundary.install(current) === undefined) throw new Error("T12 multi-branch install fell back");
  };
  for (let index = 0; index < WARMUP; index += 1) operation();
  resetRetainedIdentityCounters();
  const samples: number[] = [];
  for (let index = 0; index < MEASURED; index += 1) {
    const started = Bun.nanoseconds();
    operation();
    samples.push(Number(Bun.nanoseconds() - started));
  }
  const counters = retainedIdentityCounterSnapshot();
  boundary.close();
  host.dispose();
  return { samples, counters };
}

const session = nativeViewAbiSession();
const result = runCase(session);
const artifact = [
  JSON.stringify({
    record_kind: "t12_transaction_smoke",
    profile: "smoke",
    benchmark_version: "PERF-12",
    candidate: "retained_dag_ffi",
    workload: "multi_branch_shared_child",
    mode: "SHARED_PATH",
    warmup_ops: WARMUP,
    measured_ops: MEASURED,
    samples_ns: result.samples.map(Math.round),
    median_ns: Math.round(median(result.samples)),
    p95_ns: Math.round(percentile(result.samples, 0.95)),
    p99_ns: Math.round(percentile(result.samples, 0.99)),
    structural: result.counters,
  }),
  JSON.stringify({
    record_kind: "t12_transaction_provenance",
    profile: "smoke",
    benchmark_version: "PERF-12",
    git_sha: commandText(["git", "rev-parse", "HEAD"]),
    bun_version: Bun.version,
    bun_revision: commandText(["bun", "--revision"]),
    rustc_version: commandText(["rustc", "--version"]),
    target: commandText(["rustc", "-vV"]).split("host: ")[1]?.split("\n")[0] ?? "unknown",
    addon_sha256: commandText(["shasum", "-a", "256", "packages/iyon-runtime/native/iyon-native.node"]).split(" ")[0],
  }),
].join("\n") + "\n";
writeFileSync("packages/iyon-runtime/bench/PERF-12-t12-transaction.jsonl", artifact);
console.log(artifact);
