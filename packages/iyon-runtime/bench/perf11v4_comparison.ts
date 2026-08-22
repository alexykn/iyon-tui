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
  readonly forced_frame: Record<string, unknown>;
  readonly route_counts: Record<string, number>;
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
  const isWide = testCase.mode.startsWith("WIDE_PARENT_");
  Object.assign(environment, {
    PERF_V4_CANDIDATE: candidate,
    PERF_V4_WORKLOAD: testCase.workload,
    PERF_V4_SIZE: String(testCase.size),
    PERF_V4_MODE: testCase.mode,
    PERF_V4_LABEL: testCase.label,
    PERF_V4_ORDERING_BLOCK: String(block),
    ...(process.env.PERF_V4_WARMUP === undefined ? { PERF_V4_WARMUP: testCase.mode === "IDENTICAL_IDENTITY" ? "10000" : isWide ? "10" : "50" } : {}),
    ...(process.env.PERF_V4_MEASURED === undefined ? { PERF_V4_MEASURED: testCase.mode === "IDENTICAL_IDENTITY" ? "10000" : isWide && testCase.size >= 100_000 ? "50" : "1000" } : {}),
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
  const sizes = envNumbers("PERF_V4_SIZES", [20, 200, 2_000]);
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

function resultGroup(result: ChildResult): "normal_retained" | "text_and_special" | "wide" | "realistic_trace" {
  if (result.mode === "REALISTIC_TRACE") return "realistic_trace";
  if (result.mode.startsWith("WIDE_PARENT_")) return "wide";
  if (result.mode === "TEXT_METADATA_PATCH" || result.mode === "DECORATION_PATCH" || result.workload.startsWith("long_text") || result.workload.startsWith("large_")) return "text_and_special";
  return "normal_retained";
}

function phaseMedian(value: Record<string, unknown>): number { return Number(value.median_ns ?? 0); }

function publishedRatio(direct: ChildResult, native: ChildResult): Record<string, unknown> {
  const median = native.median_ns / Math.max(1, direct.median_ns);
  return {
    direct_7v2_median_ns: direct.median_ns,
    native_11v3_median_ns: native.median_ns,
    native_over_direct_median_ratio: median,
    native_over_direct_p95_ratio: native.p95_ns / Math.max(1, direct.p95_ns),
    native_over_direct_p99_ratio: native.p99_ns / Math.max(1, direct.p99_ns),
    native_over_direct_median_ratio_ci95: bootstrapRatio(direct.samples_ns, native.samples_ns),
    direct_7v2_construction_ns: phaseMedian(direct.construction),
    native_11v3_construction_ns: phaseMedian(native.construction),
    direct_7v2_transport_prepare_ns: phaseMedian(direct.transport_prepare),
    native_11v3_transport_prepare_ns: phaseMedian(native.transport_prepare),
    direct_7v2_native_commit_ns: phaseMedian(direct.native_commit),
    native_11v3_native_commit_ns: phaseMedian(native.native_commit),
  };
}

function aggregateMatched(rows: readonly { readonly ratio: number }[]): Record<string, unknown> {
  const ratios = rows.map((row) => row.ratio).filter((value) => Number.isFinite(value) && value > 0);
  return {
    cases: ratios.length,
    geometric_median_ratio: geometricMean(ratios),
    native_wins: ratios.filter((value) => value < 1).length,
    direct_wins: ratios.filter((value) => value > 1).length,
  };
}

function classifyTrace(ratioValue: number): string {
  if (ratioValue >= 1.05) return "D_candidate_A_wins";
  if (ratioValue > 0.95) return "C_practical_tie";
  if (ratioValue <= 0.85) return "A_11v3_decisive";
  return "B_11v3_modest_winner";
}

async function main(): Promise<void> {
  const selected = envList("PERF_V4_CANDIDATES", candidateNames).filter((value): value is Candidate => candidateNames.includes(value as Candidate));
  if (selected.length < 2) throw new Error("PERF_V4_CANDIDATES must contain at least two candidates");
  const cases = buildCases();
  const results: ChildResult[] = [];
  let block = 0;
  for (const testCase of cases) {
    const caseCandidates = testCase.mode.startsWith("WIDE_PARENT_") ? selected.filter((candidate) => candidate !== "direct_current") : selected;
    const order = block % 2 === 0 ? caseCandidates : [...caseCandidates].reverse();
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
    if (direct === undefined || native === undefined) continue;
    const ratioValue = native.median_ns / Math.max(1, direct.median_ns);
    const [workload, size, mode] = key.split("/");
    matched.push({ case: key, group: resultGroup(direct), mode, workload, size: Number(size), ratio: publishedRatio(direct, native), direct_current_median_ns: candidates.get("direct_current")?.median_ns ?? null });
  }
  const grouped = new Map<string, { ratio: number }[]>();
  const modes = new Map<string, { ratio: number }[]>();
  for (const row of matched) {
    const ratioValue = Number((row.ratio as { native_over_direct_median_ratio: number }).native_over_direct_median_ratio);
    const group = String(row.group);
    const mode = String(row.mode);
    grouped.set(group, [...(grouped.get(group) ?? []), { ratio: ratioValue }]);
    modes.set(mode, [...(modes.get(mode) ?? []), { ratio: ratioValue }]);
  }
  const constructionRows = results.map((result) => ({ case: caseKey(result), candidate: result.candidate, construction: result.construction, transport_prepare: result.transport_prepare, native_commit: result.native_commit, forced_frame: result.forced_frame }));
  const traceRatio = grouped.get("realistic_trace")?.[0]?.ratio ?? 0;
  const summary = {
    benchmark_version: "PERF-11v4",
    purpose: "Authoritative Bun 1.4 comparison of faithful PERF-7v2 Candidate A / Direct against completed PERF-11v3.",
    non_goal: "No BridgeViewNode-plus-FFI or other PERF-12 architecture is designed or implemented.",
    run_command: "bun run bench:tui-perf11v4",
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
      normal_sizes: [...new Set(cases.filter((item) => !item.mode.startsWith("WIDE_") && !item.workload.startsWith("long_") && !item.workload.startsWith("large_")).map((item) => item.size))],
      wide_sizes: [...new Set(cases.filter((item) => item.mode.startsWith("WIDE_")).map((item) => item.size))],
      process_isolation: true,
      alternating_candidate_order: true,
      raw_samples_retained: true,
      normal_warmup_iterations: 50,
      normal_measured_iterations: 1_000,
      exact_identity_warmup_iterations: 10_000,
      exact_identity_measured_iterations: 10_000,
      wide_warmup_iterations: 10,
      wide_measured_iterations: "1,000 except 100,000-child cases at 50",
    },
    record_counts: { raw_records: results.length, matched_cases: matched.length, candidate_records_by_name: Object.fromEntries(candidateNames.map((name) => [name, results.filter((result) => result.candidate === name).length])) },
    aggregates: Object.fromEntries([...grouped.entries()].map(([name, rows]) => [name, aggregateMatched(rows)])),
    mode_aggregates: Object.fromEntries([...modes.entries()].sort(([left], [right]) => left.localeCompare(right)).map(([name, rows]) => [name, aggregateMatched(rows)])),
    matched_pairs: matched,
    construction_transport_table: constructionRows,
    route_counts_in_timing: results.every((result) => Object.values(result.route_counts).every((value) => value === 0)),
    route_diagnostics_artifact: "packages/iyon-runtime/bench/PERF-11v4-route-diagnostics.json",
    route_count_note: "Timing runs disable route counters; the separate counter run is diagnostic only.",
    classification: classifyTrace(traceRatio),
    limitations: [
      ...(cases.some((item) => item.size === 10_000 && !item.mode.startsWith("WIDE_")) ? [] : ["The 10,000 normal-tree size is omitted from the full retained matrix because its cold/rebuilt 1,000-sample cost is not duration-sensible; it is covered in wide scaling."]),
      "Heap and RSS deltas are secondary process-level observations; no GC claim is made.",
    ],
  };
  const rawPath = Bun.env.PERF_V4_RAW_OUT ?? "packages/iyon-runtime/bench/PERF-11v4-comparison-raw.jsonl";
  const summaryPath = Bun.env.PERF_V4_SUMMARY_OUT ?? "packages/iyon-runtime/bench/PERF-11v4-comparison.json";
  await Bun.write(rawPath, `${results.map((result) => JSON.stringify(result)).join("\n")}\n`);
  await Bun.write(summaryPath, `${JSON.stringify(summary, null, 2)}\n`);
  console.log(JSON.stringify({ benchmark_version: "PERF-11v4", cases: cases.length, child_records: results.length, rawPath, summaryPath, aggregates: summary.aggregates }));
}

await main();
