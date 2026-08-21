import { comparisonCases, type ComparisonCase, type ComparisonMode, type ComparisonWorkload } from "./perf11v4_fixtures.ts";

const candidateNames = ["direct_7v2", "direct_current", "native_11v3"] as const;
type Candidate = (typeof candidateNames)[number];
type ChildResult = {
  readonly candidate: Candidate;
  readonly workload: string;
  readonly size: number;
  readonly mode: string;
  readonly label: string;
  readonly ordering_block: number;
  readonly samples_ns: readonly number[];
  readonly construction_samples_ns: readonly number[];
  readonly transport_prepare_samples_ns: readonly number[];
  readonly native_samples_ns: readonly number[];
  readonly median_ns: number;
  readonly p95_ns: number;
  readonly p99_ns: number;
  readonly median_ci95_ns: readonly [number, number];
  readonly p95_ci95_ns: readonly [number, number];
  readonly construction: Record<string, unknown>;
  readonly transport_prepare: Record<string, unknown>;
  readonly native_commit: Record<string, unknown>;
  readonly git_sha: string;
  readonly historical_candidate_sha: string;
  readonly git_dirty: boolean;
  readonly bun_version: string;
  readonly bun_revision: string;
  readonly rustc_version: string;
  readonly target: string;
  readonly macos_version: string;
  readonly cpu_model: string;
  readonly native_artifact_sha256: string;
};

function envList(name: string, fallback: readonly string[]): string[] {
  const raw = Bun.env[name];
  return raw === undefined || raw.trim() === "" ? [...fallback] : raw.split(",").map((value) => value.trim()).filter(Boolean);
}
function envNumbers(name: string, fallback: readonly number[]): number[] {
  return envList(name, fallback.map(String)).map(Number).filter((value) => Number.isSafeInteger(value) && value > 0);
}
function commandText(command: string[]): string { return new TextDecoder().decode(Bun.spawnSync(command).stdout).trim() || "unknown"; }
function percentile(samples: readonly number[], percentage: number): number { const sorted = [...samples].sort((a, b) => a - b); return sorted[Math.ceil((sorted.length - 1) * percentage / 100)] ?? 0; }
function geometricMean(values: readonly number[]): number { return values.length === 0 ? 0 : Math.exp(values.reduce((sum, value) => sum + Math.log(value), 0) / values.length); }
function bootstrapRatio(left: readonly number[], right: readonly number[]): readonly [number, number] {
  const count = Math.min(left.length, right.length);
  if (count < 2) return [0, 0];
  let seed = 0x11_44_7a;
  const ratios: number[] = [];
  for (let iteration = 0; iteration < 1_000; iteration += 1) {
    const sample: number[] = [];
    for (let index = 0; index < count; index += 1) {
      seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
      const selected = seed % count;
      sample.push((right[selected] ?? 0) / Math.max(1, left[selected] ?? 1));
    }
    ratios.push(percentile(sample, 50));
  }
  ratios.sort((a, b) => a - b);
  return [ratios[25]!, ratios[974]!];
}
function ratio(left: ChildResult, right: ChildResult): Record<string, unknown> {
  const medianRatio = right.median_ns / Math.max(1, left.median_ns);
  return {
    direct_candidate: left.candidate,
    compared_candidate: right.candidate,
    median_ratio: medianRatio,
    percentage_difference: (medianRatio - 1) * 100,
    p95_ratio: right.p95_ns / Math.max(1, left.p95_ns),
    median_ratio_ci95: bootstrapRatio(left.samples_ns, right.samples_ns),
  };
}
function caseKey(result: { readonly workload: string; readonly size: number; readonly mode: string }): string { return `${result.workload}/${result.size}/${result.mode}`; }

async function runChild(candidate: Candidate, testCase: ComparisonCase, block: number): Promise<ChildResult> {
  const childPath = new URL("./perf11v4_child.ts", import.meta.url).pathname;
  const environment: Record<string, string> = {};
  for (const [key, value] of Object.entries(process.env)) if (value !== undefined) environment[key] = value;
  Object.assign(environment, {
    PERF_V4_CANDIDATE: candidate,
    PERF_V4_WORKLOAD: testCase.workload,
    PERF_V4_SIZE: String(testCase.size),
    PERF_V4_MODE: testCase.mode,
    PERF_V4_LABEL: testCase.label,
    PERF_V4_ORDERING_BLOCK: String(block),
  });
  const child = Bun.spawn(["bun", "run", childPath], { env: environment, stdout: "pipe", stderr: "pipe" });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited,
  ]);
  if (exitCode !== 0) throw new Error(`PERF-11v4 child failed (${candidate} ${testCase.label}):\n${stderr}\n${stdout}`);
  const line = stdout.trim().split("\n").at(-1);
  if (line === undefined) throw new Error(`PERF-11v4 child returned no JSON (${candidate} ${testCase.label})`);
  return JSON.parse(line) as ChildResult;
}

function buildCases(): ComparisonCase[] {
  const workloads = envList("PERF_V4_WORKLOADS", [
    "plain_text_column", "styled_span_heavy", "row_heavy", "column_track_heavy", "grid_heavy", "decoration_heavy", "diff_heavy", "component_heavy", "mixed_realistic",
  ]) as ComparisonWorkload[];
  const sizes = envNumbers("PERF_V4_SIZES", [20, 200, 2_000, 10_000]);
  const cases = comparisonCases(workloads, sizes);
  if (Bun.env.PERF_V4_INCLUDE_SPECIAL !== "0") {
    const specialWorkloads: readonly ComparisonWorkload[] = ["long_text_wrap_only", "long_text_one_span_edit", "large_diff_one_hunk_edit", "large_decoration_only_change"];
    const specialSizes = envNumbers("PERF_V4_SPECIAL_SIZES", [20, 2_000]);
    for (const workload of specialWorkloads) for (const size of specialSizes) {
      for (const mode of ["COLD", "TEXT_METADATA_PATCH", "DECORATION_PATCH"] as const) cases.push({ workload, size, mode, label: `${workload}/${size}/${mode}` });
    }
  }
  if (Bun.env.PERF_V4_INCLUDE_WIDE !== "0") {
    const wideSizes = envNumbers("PERF_V4_WIDE_SIZES", [32, 256, 2_048, 10_000, 100_000]);
    for (const size of wideSizes) for (const mode of ["WIDE_PARENT_ONE_EDIT", "WIDE_PARENT_INSERT", "WIDE_PARENT_REMOVE"] as const) cases.push({ workload: "column_track_heavy", size, mode, label: `wide/${size}/${mode}` });
  }
  if (Bun.env.PERF_V4_INCLUDE_TRACE !== "0") cases.push({ workload: "mixed_realistic", size: 200, mode: "REALISTIC_TRACE" as ComparisonMode, label: "realistic_trace/1000" });
  const limit = Bun.env.PERF_V4_CASE_LIMIT === undefined ? cases.length : Number(Bun.env.PERF_V4_CASE_LIMIT);
  return cases.slice(0, Number.isSafeInteger(limit) && limit > 0 ? limit : cases.length);
}

async function main(): Promise<void> {
  const selected = envList("PERF_V4_CANDIDATES", candidateNames).filter((value): value is Candidate => candidateNames.includes(value as Candidate));
  if (selected.length < 2) throw new Error("PERF_V4_CANDIDATES must contain at least two candidates");
  const cases = buildCases();
  const results: ChildResult[] = [];
  let block = 0;
  for (const testCase of cases) {
    const order = block % 2 === 0 ? selected : [...selected].reverse();
    for (const candidate of order) results.push(await runChild(candidate, testCase, block));
    block += 1;
  }

  const byCase = new Map<string, Map<Candidate, ChildResult>>();
  for (const result of results) {
    const entry = byCase.get(caseKey(result)) ?? new Map<Candidate, ChildResult>();
    entry.set(result.candidate, result);
    byCase.set(caseKey(result), entry);
  }
  const matched: Record<string, unknown>[] = [];
  for (const [key, candidates] of byCase) {
    const direct = candidates.get("direct_7v2");
    const native = candidates.get("native_11v3");
    if (direct !== undefined && native !== undefined) matched.push({ case: key, ...ratio(direct, native) });
  }
  const ratios = matched.map((entry) => Number(entry.median_ratio)).filter((value) => Number.isFinite(value) && value > 0);
  const constructionRows = results.map((result) => ({ case: caseKey(result), candidate: result.candidate, construction: result.construction, transport_prepare: result.transport_prepare, native_commit: result.native_commit }));
  const summary = {
    benchmark_version: "PERF-11v4",
    purpose: "Bun 1.4 comparison of faithful PERF-7v2 Candidate A Direct against completed PERF-11v3",
    non_goal: "No new transport architecture is designed or implemented",
    run_command: "bun run packages/iyon-runtime/bench/perf11v4_comparison.ts",
    environment: {
      bun_version: results[0]?.bun_version ?? Bun.version,
      bun_revision: results[0]?.bun_revision ?? commandText(["bun", "--revision"]),
      rustc_version: results[0]?.rustc_version ?? commandText(["rustc", "--version"]),
      target: results[0]?.target ?? "unknown",
      macos_version: results[0]?.macos_version ?? "unknown",
      cpu_model: results[0]?.cpu_model ?? "unknown",
      current_git_sha: results[0]?.git_sha ?? commandText(["git", "rev-parse", "HEAD"]),
      historical_candidate_sha: "e5292d62c4011610850cbdc1ba4a35f296f78e4f",
      native_artifact_sha256: results[0]?.native_artifact_sha256 ?? "unknown",
      all_children_clean: results.every((result) => !result.git_dirty),
    },
    configuration: {
      cases: cases.length,
      candidates: selected,
      normal_warmup: 50,
      normal_measured: 1_000,
      exact_measured: 10_000,
      process_isolation: true,
      alternating_order: true,
      raw_samples_retained: true,
    },
    candidates: results,
    construction_transport_table: constructionRows,
    matched_pairs: matched,
    aggregate: {
      direct_7v2_vs_native_11v3_geometric_median_ratio: geometricMean(ratios),
      direct_7v2_wins_count: ratios.filter((value) => value > 1).length,
      native_11v3_wins_count: ratios.filter((value) => value < 1).length,
      practical_tie_count: ratios.filter((value) => Math.abs(value - 1) < 0.05).length,
    },
    classification: "pending_authoritative_matrix",
  };
  const rawPath = Bun.env.PERF_V4_RAW_OUT ?? "packages/iyon-runtime/bench/PERF-11v4-raw.jsonl";
  const summaryPath = Bun.env.PERF_V4_SUMMARY_OUT ?? "packages/iyon-runtime/bench/PERF-11v4-comparison.json";
  await Bun.write(rawPath, `${results.map((result) => JSON.stringify(result)).join("\n")}\n`);
  await Bun.write(summaryPath, `${JSON.stringify(summary, null, 2)}\n`);
  console.log(JSON.stringify({ benchmark_version: "PERF-11v4", cases: cases.length, child_records: results.length, rawPath, summaryPath, aggregate: summary.aggregate }));
}

await main();
