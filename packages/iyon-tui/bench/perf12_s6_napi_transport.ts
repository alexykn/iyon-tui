import { Tui, View } from "../src/index.ts";
import { nativeViewRouteSnapshot, resetNativeViewRouteCounters } from "../src/native_view_abi.ts";

const warmup = 50;
const samples = 200;
const sizes = [20, 200] as const;
const candidate = process.env.PERF12_CANDIDATE ?? "napi";
const gitSha = process.env.PERF12_GIT_SHA ?? "unknown";
const nativeArtifactSha256 = process.env.PERF12_NATIVE_SHA256 ?? "unknown";
const rustcVersion = process.env.PERF12_RUSTC_VERSION ?? "unknown";
const target = process.env.PERF12_TARGET ?? "unknown";
const requestedWorkload = process.env.PERF12_WORKLOAD;
const requestedSize = process.env.PERF12_SIZE === undefined ? undefined : Number(process.env.PERF12_SIZE);

type Workload = "shared_path" | "text_layout";

function stableSubtree(size: number): View {
  return View.vertical(Array.from({ length: size }, (_, index) => View.text(`stable-${index}`).noWrap()));
}

function median(values: readonly number[]): number {
  const ordered = [...values].sort((left, right) => left - right);
  return ordered[Math.floor(ordered.length / 2)]!;
}

function percentile(values: readonly number[], percentileValue: number): number {
  const ordered = [...values].sort((left, right) => left - right);
  return ordered[Math.min(ordered.length - 1, Math.ceil(ordered.length * percentileValue) - 1)]!;
}

function bootstrapMedianInterval(values: readonly number[], rounds = 200): [number, number] {
  const medians: number[] = [];
  for (let round = 0; round < rounds; round++) {
    const resample = Array.from({ length: values.length }, () => values[Math.floor(Math.random() * values.length)]!);
    medians.push(median(resample));
  }
  medians.sort((left, right) => left - right);
  return [medians[Math.floor(medians.length * 0.025)]!, medians[Math.floor(medians.length * 0.975)]!];
}

async function measure(workload: Workload, size: number): Promise<Record<string, unknown>> {
  const tui = await Tui.open({ width: 80, height: 24, headless: true });
  const stable = stableSubtree(size);
  const layoutStable = stableSubtree(Math.max(1, Math.min(size, 20)));
  let changingText = View.text("initial").noWrap();
  let body = workload === "shared_path"
    ? View.vertical([changingText, stable])
    : View.vertical([changingText, layoutStable]);
  tui.render({ body });
  const nextBody = (index: number): View => {
    if (workload === "shared_path") {
      changingText = View.text(`changed-${index}`).noWrap();
      return View.vertical([changingText, stable]);
    }
    changingText = changingText.textAlign(index % 2 === 0 ? "center" : "end");
    return View.vertical([changingText, layoutStable]);
  };
  resetNativeViewRouteCounters();
  for (let index = 0; index < warmup; index++) {
    body = nextBody(index);
    tui.render({ body });
  }
  resetNativeViewRouteCounters();
  const measured: number[] = [];
  for (let index = 0; index < samples; index++) {
    const start = Bun.nanoseconds();
    body = nextBody(warmup + index);
    tui.render({ body });
    measured.push(Bun.nanoseconds() - start);
  }
  const routes = nativeViewRouteSnapshot();
  tui.close();
  return {
    benchmark_version: "PERF-12-S6",
    profile: "smoke",
    candidate,
    transport: candidate === "napi" ? "generated_safe_napi" : "legacy_direct_ffi",
    workload,
    size,
    git_sha: gitSha,
    bun_version: Bun.version,
    bun_revision: Bun.revision,
    rustc_version: rustcVersion,
    target,
    native_artifact_sha256: nativeArtifactSha256,
    warmup,
    measured: samples,
    process_isolated: requestedWorkload !== undefined && requestedSize !== undefined,
    samples_ns: measured,
    median_ns: median(measured),
    p95_ns: percentile(measured, 0.95),
    p99_ns: percentile(measured, 0.99),
    median_ci95_ns: bootstrapMedianInterval(measured),
    routes,
  };
}

const workloads = requestedWorkload === undefined
  ? (["shared_path", "text_layout"] as const)
  : [requestedWorkload as Workload];
const selectedSizes = requestedSize === undefined ? sizes : [requestedSize];
for (const workload of workloads) {
  for (const size of selectedSizes) {
    console.log(JSON.stringify(await measure(workload, size)));
  }
}
