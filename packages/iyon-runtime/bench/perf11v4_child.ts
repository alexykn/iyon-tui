import { native } from "../src/native.ts";
import { Tui } from "../src/tui/index.ts";
import { nativeViewRouteSnapshot, resetNativeViewRouteCounters } from "../src/tui/native_view_abi.ts";
import { nodeForDirectBridge, View } from "../src/tui/values/view.ts";
import { nodeForPerf7v2Bridge, Perf7v2View } from "./perf7v2_direct/view.ts";
import { buildTracePair, prepareComparisonCase, setComparisonComponentId, type ComparisonMode, type ComparisonWorkload } from "./perf11v4_fixtures.ts";

const candidate = requiredEnv("PERF_V4_CANDIDATE") as "direct_7v2" | "direct_current" | "native_11v3";
if (!["direct_7v2", "direct_current", "native_11v3"].includes(candidate)) throw new Error(`unknown candidate: ${candidate}`);
const workload = requiredEnv("PERF_V4_WORKLOAD") as ComparisonWorkload;
const size = positiveEnv("PERF_V4_SIZE");
const mode = requiredEnv("PERF_V4_MODE") as ComparisonMode | "REALISTIC_TRACE";
const label = Bun.env.PERF_V4_LABEL ?? `${workload}/${size}/${mode}`;
const warmup = positiveEnv("PERF_V4_WARMUP", 50);
const measured = positiveEnv("PERF_V4_MEASURED", mode === "IDENTICAL_IDENTITY" ? 10_000 : 1_000);
const repeat = positiveEnv("PERF_V4_REPEAT", 1);
const orderingBlock = Number(Bun.env.PERF_V4_ORDERING_BLOCK ?? 0);

interface NativeHost {
  render(view: object): void;
  screenRows(): string[];
  dispose(): void;
  createViewSlot(initial: object): { componentId(): number | null; dispose?(): void };
}

function requiredEnv(name: string): string {
  const value = Bun.env[name];
  if (value === undefined || value === "") throw new Error(`${name} is required`);
  return value;
}

function positiveEnv(name: string, fallback?: number): number {
  const value = Number(Bun.env[name] ?? fallback);
  if (!Number.isSafeInteger(value) || value <= 0) throw new Error(`${name} must be a positive integer`);
  return value;
}

function commandText(command: string[]): string {
  const result = Bun.spawnSync(command);
  return new TextDecoder().decode(result.stdout).trim() || "unknown";
}

function gitSha(): string { return commandText(["git", "rev-parse", "HEAD"]); }
function gitDirty(): boolean {
  const result = Bun.spawnSync(["git", "status", "--porcelain"]);
  return new TextDecoder().decode(result.stdout).trim() !== "";
}
function sha256(path: string): string { return commandText(["shasum", "-a", "256", path]).split(/\s+/)[0] ?? "unknown"; }
function now(): number { return Bun.nanoseconds(); }

function percentile(samples: readonly number[], percentage: number): number {
  const sorted = [...samples].sort((left, right) => left - right);
  return sorted[Math.ceil((sorted.length - 1) * percentage / 100)] ?? 0;
}

function bootstrapInterval(samples: readonly number[], percentage: number): readonly [number, number] {
  if (samples.length < 2) return [samples[0] ?? 0, samples[0] ?? 0];
  let seed = 0x4f11_0001;
  const estimates: number[] = [];
  for (let iteration = 0; iteration < 1_000; iteration += 1) {
    const resample: number[] = [];
    for (let index = 0; index < samples.length; index += 1) {
      seed = (Math.imul(seed, 1_664_525) + 1_013_904_223) >>> 0;
      resample.push(samples[seed % samples.length]!);
    }
    estimates.push(percentile(resample, percentage));
  }
  estimates.sort((left, right) => left - right);
  return [estimates[25]!, estimates[974]!];
}

function stats(samples: readonly number[]): Record<string, unknown> {
  return {
    median_ns: percentile(samples, 50),
    p95_ns: percentile(samples, 95),
    p99_ns: percentile(samples, 99),
    median_ci95_ns: bootstrapInterval(samples, 50),
    p95_ci95_ns: bootstrapInterval(samples, 95),
  };
}

function createHost(): NativeHost {
  const Host = native.NativeTuiHost;
  if (Host === undefined) throw new Error("NativeTuiHost is unavailable");
  return new Host(80, 24, true) as unknown as NativeHost;
}

function bridgeFor(view: View | Perf7v2View): object {
  if (candidate === "direct_7v2") return nodeForPerf7v2Bridge(view as Perf7v2View);
  return nodeForDirectBridge(view as View);
}

async function main(): Promise<void> {
  const host = candidate === "native_11v3" ? undefined : createHost();
  const tui = candidate === "native_11v3" ? await Tui.open({ width: 80, height: 24, headless: true }) : undefined;
  const componentHost = candidate === "native_11v3" ? (tui as unknown as { readonly host: NativeHost }).host : host;
  const componentSlot = workload === "component_heavy" ? componentHost!.createViewSlot(nodeForDirectBridge(View.spacer(0))) : undefined;
  if (componentSlot?.componentId() !== null && componentSlot?.componentId() !== undefined) setComparisonComponentId(componentSlot.componentId()!);
  const trace = mode === "REALISTIC_TRACE";
  const prepared = trace ? undefined : prepareComparisonCase<View | Perf7v2View>(candidate === "direct_7v2" ? "perf7v2" : "current", { workload, size, mode: mode as ComparisonMode, label });
  if (prepared?.base !== undefined) {
    if (candidate === "native_11v3") tui!.render({ body: prepared.base as View });
    else host!.render(bridgeFor(prepared.base));
  }
  const totalSamples: number[] = [];
  const constructionSamples: number[] = [];
  const prepareSamples: number[] = [];
  const nativeSamples: number[] = [];
  const forcedFrameSamples: number[] = [];
  let lastRows: readonly string[] = [];
  const traceDistribution: Record<string, number> = {};
  const routeCounts: Record<string, number> = {};
  const cpuBefore = process.cpuUsage?.();
  const rssBefore = process.memoryUsage().rss;
  try {
    const sampleCount = warmup + measured;
    for (let repetition = 0; repetition < repeat; repetition += 1) {
      for (let index = 0; index < sampleCount; index += 1) {
        const sampleIndex = repetition * sampleCount + index;
        if (index >= warmup) resetNativeViewRouteCounters();
        const constructionStarted = now();
        const tracePair = trace ? buildTracePair<View | Perf7v2View>(candidate === "direct_7v2" ? "perf7v2" : "current", sampleIndex) : undefined;
        const next = tracePair?.next ?? prepared!.next(sampleIndex);
        const constructionNs = now() - constructionStarted;
        if (tracePair !== undefined && !tracePair.cold) {
          if (candidate === "native_11v3") tui!.render({ body: tracePair.base as View });
          else host!.render(bridgeFor(tracePair.base));
        }
        const transportStarted = now();
        let nativeStarted = transportStarted;
        if (candidate === "native_11v3") {
          const commitStarted = now();
          tui!.render({ body: next as View });
          nativeStarted = commitStarted;
        } else {
          const bridged = bridgeFor(next);
          nativeStarted = now();
          host!.render(bridged);
        }
        const end = now();
        const prepareNs = candidate === "native_11v3" ? 0 : nativeStarted - transportStarted;
        const nativeNs = end - nativeStarted;
        if (index >= warmup) {
          for (const [route, count] of Object.entries(nativeViewRouteSnapshot())) routeCounts[route] = (routeCounts[route] ?? 0) + count;
          if (tracePair !== undefined) traceDistribution[tracePair.category] = (traceDistribution[tracePair.category] ?? 0) + 1;
          totalSamples.push(constructionNs + prepareNs + nativeNs);
          constructionSamples.push(constructionNs);
          prepareSamples.push(prepareNs);
          nativeSamples.push(nativeNs);
          forcedFrameSamples.push(nativeNs);
        }
        if (index === sampleCount - 1) {
          lastRows = candidate === "native_11v3" ? tui!.screenRows() : host!.screenRows();
        }
      }
    }
  } finally {
    componentSlot?.dispose?.();
    tui?.close();
    host?.dispose();
  }
  const cpuAfter = process.cpuUsage?.();
  const cpuUserUs = cpuBefore !== undefined && cpuAfter !== undefined ? (cpuAfter.user - cpuBefore.user) : 0;
  const cpuSystemUs = cpuBefore !== undefined && cpuAfter !== undefined ? (cpuAfter.system - cpuBefore.system) : 0;
  const rssDelta = Math.max(0, process.memoryUsage().rss - rssBefore);
  const nativeArtifact = new URL("../native/iyon-native.node", import.meta.url).pathname;
  const result = {
    benchmark_version: "PERF-11v4",
    candidate,
    workload,
    size,
    mode,
    label,
    ordering_block: orderingBlock,
    git_sha: gitSha(),
    historical_candidate_sha: "e5292d62c4011610850cbdc1ba4a35f296f78e4f",
    git_dirty: gitDirty(),
    bun_version: Bun.version,
    bun_revision: commandText(["bun", "--revision"]),
    rustc_version: commandText(["rustc", "--version"]),
    target: commandText(["rustc", "-vV"]).split("\n").find((line) => line.startsWith("host:"))?.split(/\s+/)[1] ?? "unknown",
    macos_version: commandText(["sw_vers", "-productVersion"]),
    cpu_model: commandText(["sysctl", "-n", "hw.model"]),
    native_artifact_sha256: sha256(nativeArtifact),
    warmup_iterations: warmup,
    measured_iterations: measured,
    repeats: repeat,
    samples_ns: totalSamples,
    construction_samples_ns: constructionSamples,
    transport_prepare_samples_ns: prepareSamples,
    native_samples_ns: nativeSamples,
    forced_frame_samples_ns: forcedFrameSamples,
    ...stats(totalSamples),
    construction: stats(constructionSamples),
    transport_prepare: stats(prepareSamples),
    native_commit: stats(nativeSamples),
    forced_frame: stats(forcedFrameSamples),
    structural_encoding_ns: 0,
    bytes_written_to_transport: 0,
    op_records_written: 0,
    command_words_written: 0,
    path_arrays_written: 0,
    node_id_arrays_written: 0,
    cpu_user_us: cpuUserUs,
    cpu_system_us: cpuSystemUs,
    rss_delta_bytes: rssDelta,
    last_screen_rows: lastRows,
    route_counts: routeCounts,
    ...(trace ? { trace_operations: totalSamples.length, trace_distribution: traceDistribution } : {}),
  };
  process.stdout.write(`${JSON.stringify(result)}\n`);
}

await main();
