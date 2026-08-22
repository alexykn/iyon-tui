/**
 * PERF-12.0 memory attribution parent (PERF-12 handoff §57–§58).
 *
 * Reproduces the ~2.7 GiB-class PERF-11v4 blocks in fresh child processes
 * against the frozen native artifact and records the §57 counter set per
 * phase. Emits one JSONL record per block plus a summary record; §58 bucket
 * classification conclusions are drawn in PERF-12-baseline.md from these raw
 * numbers.
 */

import { type ComparisonMode, type ComparisonWorkload } from "./perf11v4_fixtures.ts";

interface Block {
  readonly label: string;
  readonly kind: "retained" | "first_use" | "trace";
  readonly workload?: ComparisonWorkload;
  readonly size?: number;
  readonly mode?: ComparisonMode;
  readonly ops: number;
}

// Blocks chosen to cover every ≥2.7 GiB peak-RSS class observed in the frozen
// PERF-11v4-comparison-raw.jsonl plus a small control block.
const BLOCKS: readonly Block[] = [
  { label: "control/mixed_realistic/20/IDENTICAL_IDENTITY", kind: "retained", workload: "mixed_realistic", size: 20, mode: "IDENTICAL_IDENTITY", ops: 500 },
  { label: "plain_text_column/2000/FIRST_USE", kind: "first_use", workload: "plain_text_column", size: 2_000, mode: "FIRST_USE", ops: 1_000 },
  { label: "row_heavy/2000/FIRST_USE", kind: "first_use", workload: "row_heavy", size: 2_000, mode: "FIRST_USE", ops: 1_000 },
  { label: "plain_text_column/10000/SHARED_DEEP", kind: "retained", workload: "plain_text_column", size: 10_000, mode: "SHARED_DEEP", ops: 1_000 },
  { label: "wide/100000/WIDE_PARENT_ONE_EDIT", kind: "retained", workload: "column_track_heavy", size: 100_000, mode: "WIDE_PARENT_ONE_EDIT", ops: 1_000 },
  { label: "realistic_trace/1000", kind: "trace", workload: "mixed_realistic", size: 200, ops: 1_000 },
];

interface ResidualCounters {
  readonly rss_bytes: number;
  readonly js_heap_used_bytes: number;
  readonly semantic_cache_entries: number;
  readonly semantic_cache_live: number;
  readonly native_ref_slots: number;
  readonly leased_slots: number;
}

interface ChildRecord {
  readonly record_kind: string;
  readonly label?: string;
  readonly residual_after_cleanup?: ResidualCounters;
  readonly [key: string]: unknown;
}

async function runChild(block: Block): Promise<ChildRecord> {
  // One bounded retry: a hard kill right after a multi-GiB predecessor exits
  // can be transient jetsam pressure; retry once before failing the run.
  try {
    return await runChildOnce(block);
  } catch (error) {
    process.stdout.write(`  child failed (${(error as Error).message.split("\n")[0]}); retrying once\n`);
    await new Promise((resolve) => setTimeout(resolve, 2_000));
    return await runChildOnce(block);
  }
}

async function runChildOnce(block: Block): Promise<ChildRecord> {
  const childPath = new URL("./perf12_memory_child.ts", import.meta.url).pathname;
  const environment: Record<string, string> = {};
  for (const [key, value] of Object.entries(process.env)) if (value !== undefined) environment[key] = value;
  environment.PERF12_BLOCK_JSON = JSON.stringify(block);
  const child = Bun.spawn(["bun", "run", childPath], { env: environment, stdout: "pipe", stderr: "pipe" });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited,
  ]);
  if (exitCode !== 0) throw new Error(`PERF-12 memory child failed (${block.label}):\n${stderr}\n${stdout}`);
  const line = stdout.trim().split("\n").at(-1);
  if (line === undefined) throw new Error(`PERF-12 memory child returned no JSON (${block.label})`);
  return JSON.parse(line) as ChildRecord;
}

function commandText(command: string[]): string {
  return new TextDecoder().decode(Bun.spawnSync(command).stdout).trim() || "unknown";
}

async function main(): Promise<void> {
  const outPath = Bun.env.PERF12_MEMORY_OUT ?? "packages/iyon-runtime/bench/PERF-12-memory-attribution.jsonl";
  const records: ChildRecord[] = [];
  for (const block of BLOCKS) {
    process.stdout.write(`running block ${block.label} (${block.ops} ops)...\n`);
    const record = await runChild(block);
    records.push(record);
    const residual = record.residual_after_cleanup!;
    if (residual === undefined) throw new Error(`child returned no residual counters (${block.label})`);
    process.stdout.write(
      `  post-cleanup: rss=${(residual.rss_bytes! / 2 ** 20).toFixed(1)} MiB, heap=${(residual.js_heap_used_bytes! / 2 ** 20).toFixed(1)} MiB,`
      + ` cache=${residual.semantic_cache_entries} (live ${residual.semantic_cache_live}), slots=${residual.native_ref_slots} (leased ${residual.leased_slots})\n`,
    );
  }
  const summary: ChildRecord = {
    record_kind: "memory_attribution_summary",
    blocks: records.length,
    protocol: "release/close roots -> Bun.gc(true) -> native full weak sweep (tuiViewAbiBootstrap(prune=true)) -> Bun.gc(true)",
    note: "§58 classification is derived in PERF-12-baseline.md from these raw numbers; RSS alone decides nothing.",
  };
  await Bun.write(outPath, [...records, summary].map((record) => JSON.stringify(record)).join("\n") + "\n");
  console.log(JSON.stringify({ probe: "perf12_memory_attribution", blocks: records.length, outPath }));
}

await main();
