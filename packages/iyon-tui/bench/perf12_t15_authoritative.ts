import { createHash } from "node:crypto";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

const PACKAGE_ROOT = fileURLToPath(new URL("../", import.meta.url));
const CASE_RUNNER = fileURLToPath(new URL("./perf12_t15_case.ts", import.meta.url));
const WORKLOADS = [
  "plain_text_column",
  "styled_span_heavy",
  "row_heavy",
  "column_track_heavy",
  "grid_heavy",
  "decoration_heavy",
  "diff_heavy",
  "component_heavy",
  "mixed_realistic",
] as const;
const MODES = [
  "exact_identity",
  "shared_path",
  "shared_deep_4",
  "shared_deep_16",
  "shared_deep_64",
  "shared_deep_128",
  "large_shared_subtree_cutoff",
  "text_metadata_patch",
  "decoration_patch",
  "rebuilt_equivalent",
] as const;
const BASE_SIZES = [20, 200, 2_000] as const;
const LARGE_WORKLOADS = new Set([
  "plain_text_column",
  "row_heavy",
  "column_track_heavy",
  "grid_heavy",
  "component_heavy",
  "mixed_realistic",
]);
const LARGE_MODES = new Set([
  "shared_path",
  "shared_deep_16",
  "large_shared_subtree_cutoff",
  "rebuilt_equivalent",
]);
const TINY_MODES = new Set(["exact_identity", "text_metadata_patch", "decoration_patch"]);

type Transport = "generated_safe_napi" | "feature_gated_direct_ffi";
type Candidate = "napi_default" | "direct_ffi_oracle";

interface CaseDefinition {
  readonly workload: string;
  readonly mode: string;
  readonly size: number;
  readonly warmup: number;
  readonly measured: number;
}

interface ResultRecord {
  readonly candidate: Candidate;
  readonly transport: Transport;
  readonly workload: string;
  readonly mode: string;
  readonly size: number;
  readonly median_ns: number;
  readonly p95_ns: number;
  readonly p99_ns: number;
  readonly median_ci95_ns: readonly [number, number];
  readonly structural_delta: Record<string, number>;
  readonly first_screen_row: string;
  readonly screen_rows: readonly string[];
  readonly semantic_node_ids_created: number;
  readonly [key: string]: unknown;
}

interface CaseComparison {
  readonly key: string;
  readonly workload: string;
  readonly mode: string;
  readonly size: number;
  readonly napi_median_ns: number;
  readonly direct_median_ns: number;
  readonly napi_over_direct: number;
  readonly correctness_match: boolean;
  readonly structural_delta_match: boolean;
}

function parseList(name: string, fallback: readonly string[]): string[] {
  const value = process.env[name];
  return value === undefined || value.trim() === ""
    ? [...fallback]
    : value.split(",").map((item) => item.trim()).filter((item) => item.length > 0);
}

function parseNumbers(name: string, fallback: readonly number[]): number[] {
  const value = process.env[name];
  return value === undefined || value.trim() === ""
    ? [...fallback]
    : value.split(",").map((item) => Number(item.trim())).filter((item) => Number.isSafeInteger(item) && item > 0);
}

function run(command: string[], cwd: string, env?: Record<string, string>): { exitCode: number; stdout: string; stderr: string } {
  const result = Bun.spawnSync({
    cmd: command,
    cwd,
    env: { ...process.env, ...env },
    stdout: "pipe",
    stderr: "pipe",
  });
  return {
    exitCode: result.exitCode,
    stdout: result.stdout === undefined ? "" : new TextDecoder().decode(result.stdout),
    stderr: result.stderr === undefined ? "" : new TextDecoder().decode(result.stderr),
  };
}

function checked(command: string[], cwd: string, env?: Record<string, string>): string {
  const result = run(command, cwd, env);
  if (result.exitCode !== 0) throw new Error(`command failed (${result.exitCode}): ${command.join(" ")}\n${result.stderr}`);
  return result.stdout.trim();
}

function gitSha(): string {
  return checked(["git", "rev-parse", "HEAD"], PACKAGE_ROOT);
}

function sourceIsClean(): boolean {
  return checked(["git", "status", "--porcelain"], PACKAGE_ROOT) === "";
}

function rustcVersion(): string {
  return checked(["rustc", "--version"], PACKAGE_ROOT);
}

function targetTriple(): string {
  const verbose = checked(["rustc", "-vV"], PACKAGE_ROOT);
  return verbose.split(/\r?\n/).find((line) => line.startsWith("host:"))?.slice("host:".length).trim() ?? "unknown";
}

async function addonSha256(): Promise<string> {
  const bytes = await readFile(new URL("../native/iyon-tui-native.node", import.meta.url));
  return createHash("sha256").update(bytes).digest("hex");
}

function cases(): CaseDefinition[] {
  const workloads = parseList("T15_WORKLOADS", WORKLOADS);
  const modes = parseList("T15_MODES", MODES);
  const sizes = parseNumbers("T15_SIZES", BASE_SIZES);
  const warmupOverride = process.env.T15_WARMUP === undefined ? undefined : Number(process.env.T15_WARMUP);
  const measuredOverride = process.env.T15_MEASURED === undefined ? undefined : Number(process.env.T15_MEASURED);
  const result: CaseDefinition[] = [];
  const includeLarge = process.env.T15_INCLUDE_LARGE !== "0";
  for (const workload of workloads) {
    for (const mode of modes) {
      for (const size of sizes) {
        const tiny = TINY_MODES.has(mode);
        result.push({
          workload,
          mode,
          size,
          warmup: warmupOverride ?? (tiny ? 10_000 : 50),
          measured: measuredOverride ?? (tiny ? 10_000 : 1_000),
        });
      }
      if (includeLarge && LARGE_WORKLOADS.has(workload) && LARGE_MODES.has(mode) && !sizes.includes(10_000)) {
        result.push({
          workload,
          mode,
          size: 10_000,
          warmup: warmupOverride ?? 50,
          measured: measuredOverride ?? 1_000,
        });
      }
    }
  }

  if (process.env.T15_INCLUDE_SPECIAL !== "0") {
    const special: readonly { readonly workload: string; readonly mode: string; readonly sizes: readonly number[] }[] = [
      { workload: "wide_axis", mode: "wide_axis_set", sizes: [32, 256, 2_048, 10_000, 100_000] },
      { workload: "wide_axis", mode: "wide_axis_splice", sizes: [32, 256, 2_048, 10_000, 100_000] },
      { workload: "wide_grid", mode: "wide_grid_cell", sizes: [32, 256, 2_048, 10_000] },
      { workload: "path_scalar", mode: "path_scalar", sizes: [20, 200, 2_000] },
    ];
    for (const definition of special) {
      for (const size of definition.sizes) {
        result.push({
          workload: definition.workload,
          mode: definition.mode,
          size,
          warmup: warmupOverride ?? 50,
          measured: measuredOverride ?? 1_000,
        });
      }
    }
  }
  return result;
}

function caseKey(definition: Pick<CaseDefinition, "workload" | "mode" | "size">): string {
  return `${definition.workload}|${definition.mode}|${definition.size}`;
}

function stage(transport: Transport): void {
  const env: Record<string, string> = transport === "feature_gated_direct_ffi" ? { ION_NATIVE_FEATURES: "direct-ffi" } : {};
  const result = run(["bun", "run", "scripts/stage-native.ts"], PACKAGE_ROOT, env);
  if (result.stdout.length > 0) process.stdout.write(result.stdout);
  if (result.stderr.length > 0) process.stderr.write(result.stderr);
  if (result.exitCode !== 0) throw new Error(`native staging failed for ${transport}`);
}

function candidateFor(transport: Transport): Candidate {
  return transport === "generated_safe_napi" ? "napi_default" : "direct_ffi_oracle";
}

function runCase(
  definition: CaseDefinition,
  profile: string,
  transport: Transport,
  sourceSha: string,
  rustc: string,
  target: string,
  addonSha: string,
): ResultRecord {
  const candidate = candidateFor(transport);
  const env: Record<string, string> = {
    T15_PROFILE: profile,
    T15_CANDIDATE: candidate,
    T15_TRANSPORT: transport,
    T15_WORKLOAD: definition.workload,
    T15_MODE: definition.mode,
    T15_SIZE: String(definition.size),
    T15_WARMUP: String(definition.warmup),
    T15_MEASURED: String(definition.measured),
    T15_GIT_SHA: sourceSha,
    T15_RUSTC_VERSION: rustc,
    T15_TARGET: target,
    T15_NATIVE_SHA256: addonSha,
  };
  const result = run(["bun", "run", CASE_RUNNER, "--", definition.workload], PACKAGE_ROOT, env);
  if (result.exitCode !== 0) {
    throw new Error(`${candidate} case ${caseKey(definition)} failed:\n${result.stderr}`);
  }
  const lines = result.stdout.trim().split(/\r?\n/).filter((line) => line.trim().length > 0);
  if (lines.length !== 1) throw new Error(`${candidate} case ${caseKey(definition)} emitted ${lines.length} result lines`);
  const record = JSON.parse(lines[0]!) as ResultRecord;
  if (record.profile !== profile || record.candidate !== candidate || record.transport !== transport) {
    throw new Error(`${candidate} case ${caseKey(definition)} returned incompatible metadata`);
  }
  return record;
}

function structuralEqual(left: Record<string, number>, right: Record<string, number>): boolean {
  const keys = new Set([...Object.keys(left), ...Object.keys(right)]);
  return [...keys].every((key) => left[key] === right[key]);
}

function geometricMean(values: readonly number[]): number {
  if (values.length === 0) return Number.NaN;
  return Math.exp(values.reduce((sum, value) => sum + Math.log(value), 0) / values.length);
}

function compare(results: readonly ResultRecord[], definitions: readonly CaseDefinition[]): {
  comparisons: CaseComparison[];
  workloadGeometricMeans: Record<string, number>;
  overallGeometricMean: number;
  correctnessFailures: string[];
} {
  const byKey = new Map<string, Map<Candidate, ResultRecord>>();
  for (const result of results) {
    const key = caseKey(result);
    const candidates = byKey.get(key) ?? new Map<Candidate, ResultRecord>();
    candidates.set(result.candidate, result);
    byKey.set(key, candidates);
  }

  const comparisons: CaseComparison[] = [];
  const correctnessFailures: string[] = [];
  for (const definition of definitions) {
    const key = caseKey(definition);
    const pair = byKey.get(key);
    const napi = pair?.get("napi_default");
    const direct = pair?.get("direct_ffi_oracle");
    if (napi === undefined || direct === undefined) {
      correctnessFailures.push(`${key}: missing candidate result`);
      continue;
    }
    const structuralDeltaMatch = structuralEqual(napi.structural_delta, direct.structural_delta);
    const screenMatch = JSON.stringify(napi.screen_rows) === JSON.stringify(direct.screen_rows);
    const semanticNodeIdCountMatch = napi.semantic_node_ids_created === direct.semantic_node_ids_created;
    const correctnessMatch = structuralDeltaMatch
      && screenMatch
      && semanticNodeIdCountMatch
      && napi.first_screen_row === direct.first_screen_row;
    if (!correctnessMatch) correctnessFailures.push(`${key}: structural, rendered-screen, or semantic NodeId mismatch`);
    comparisons.push({
      key,
      workload: definition.workload,
      mode: definition.mode,
      size: definition.size,
      napi_median_ns: napi.median_ns,
      direct_median_ns: direct.median_ns,
      napi_over_direct: napi.median_ns / direct.median_ns,
      correctness_match: correctnessMatch,
      structural_delta_match: structuralDeltaMatch,
    });
  }

  const byWorkload = new Map<string, number[]>();
  for (const comparison of comparisons) {
    const values = byWorkload.get(comparison.workload) ?? [];
    values.push(comparison.napi_over_direct);
    byWorkload.set(comparison.workload, values);
  }
  const workloadGeometricMeans = Object.fromEntries(
    [...byWorkload.entries()].map(([workload, values]) => [workload, geometricMean(values)]),
  );
  return {
    comparisons,
    workloadGeometricMeans,
    overallGeometricMean: geometricMean(Object.values(workloadGeometricMeans)),
    correctnessFailures,
  };
}

async function main(): Promise<void> {
  const profile = process.env.T15_PROFILE ?? "authoritative";
  if (profile === "authoritative" && !sourceIsClean()) {
    throw new Error("authoritative T15 requires a clean committed checkout");
  }
  const definitions = cases();
  if (definitions.length === 0) throw new Error("T15 matrix is empty");
  const sourceSha = gitSha();
  const rustc = rustcVersion();
  const target = targetTriple();
  const outputDirectory = fileURLToPath(new URL("./", import.meta.url));
  const rawPath = fileURLToPath(new URL(`./PERF-12-T15-authoritative-${sourceSha.slice(0, 12)}.jsonl`, import.meta.url));
  const summaryPath = fileURLToPath(new URL(`./PERF-12-T15-authoritative-${sourceSha.slice(0, 12)}.json`, import.meta.url));
  await mkdir(outputDirectory, { recursive: true });
  await writeFile(rawPath, "");

  const results: ResultRecord[] = [];
  const workloads = [...new Set(definitions.map((definition) => definition.workload))];
  for (const [workloadIndex, workload] of workloads.entries()) {
    const block = definitions.filter((definition) => definition.workload === workload);
    const firstTransport: Transport = workloadIndex % 2 === 0 ? "generated_safe_napi" : "feature_gated_direct_ffi";
    const secondTransport: Transport = firstTransport === "generated_safe_napi" ? "feature_gated_direct_ffi" : "generated_safe_napi";
    for (const transport of [firstTransport, secondTransport]) {
      stage(transport);
      const addonSha = await addonSha256();
      const ordered = transport === firstTransport ? block : [...block].reverse();
      console.log(`T15 ${transport}: ${workload} (${ordered.length} cases)`);
      for (const definition of ordered) {
        const record = runCase(definition, profile, transport, sourceSha, rustc, target, addonSha);
        results.push(record);
        await appendFile(rawPath, `${JSON.stringify(record)}\n`);
      }
    }
  }

  const comparison = compare(results, definitions);
  const summary = {
    benchmark_version: "PERF-12-T15",
    profile,
    status: comparison.correctnessFailures.length === 0 ? "complete" : "failed_correctness",
    recommendation: "owner_decision_required",
    source_sha: sourceSha,
    raw_jsonl: rawPath,
    workload_count: workloads.length,
    case_count_per_arm: definitions.length,
    result_count: results.length,
    candidates: ["napi_default", "direct_ffi_oracle"],
    matrix: {
      workloads,
      modes: [...new Set(definitions.map((definition) => definition.mode))],
      sizes: [...new Set(definitions.map((definition) => definition.size))].sort((a, b) => a - b),
    },
    correctness_failures: comparison.correctnessFailures,
    workload_geometric_mean_napi_over_direct: comparison.workloadGeometricMeans,
    overall_geometric_mean_napi_over_direct: comparison.overallGeometricMean,
    comparisons: comparison.comparisons,
  };
  await writeFile(summaryPath, `${JSON.stringify(summary, null, 2)}\n`);
  console.log(JSON.stringify({ summaryPath, rawPath, status: summary.status, caseCountPerArm: definitions.length }));
  if (comparison.correctnessFailures.length > 0) process.exitCode = 1;
}

await main();
