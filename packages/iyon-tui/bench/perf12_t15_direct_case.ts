import { View } from "../src/index.ts";
import { nodeForBridge } from "../src/transport/structural/view-bridge.ts";
import {
  resetRetainedIdentityCounters,
  retainedIdentityCounterSnapshot,
  RetainedRootBoundary,
  setRetainedPhaseInstrumentation,
  setRootColdMaterializer,
  type RetainedPhaseSample,
} from "./direct_ffi/retained_dag.ts";
import {
  nativeViewAbiSession,
  tryNativeMaterialize,
} from "./direct_ffi/native_view_abi.ts";
import { native } from "./direct_ffi/native.ts";
import { makeT15Scenario } from "./perf12_t15_workload.ts";

const workload = process.env.T15_WORKLOAD ?? "plain_text";
const mode = process.env.T15_MODE ?? "shared_path";
const size = Number(process.env.T15_SIZE ?? 20);
const warmup = Number(process.env.T15_WARMUP ?? 50);
const measured = Number(process.env.T15_MEASURED ?? 1_000);

function median(values: readonly number[]): number {
  const ordered = [...values].sort((left, right) => left - right);
  return ordered[Math.floor(ordered.length / 2)]!;
}
function percentile(values: readonly number[], fraction: number): number {
  const ordered = [...values].sort((left, right) => left - right);
  return ordered[Math.min(ordered.length - 1, Math.ceil(ordered.length * fraction) - 1)]!;
}
function bootstrap(values: readonly number[], rounds = 1_000): [number, number] {
  const medians: number[] = [];
  for (let round = 0; round < rounds; round++) {
    const sample = Array.from({ length: values.length }, () => values[Math.floor(Math.random() * values.length)]!);
    medians.push(median(sample));
  }
  medians.sort((left, right) => left - right);
  return [medians[Math.floor(rounds * 0.025)]!, medians[Math.floor(rounds * 0.975)]!];
}

const Host = native.NativeTuiHost;
if (Host === undefined) throw new Error("direct-ffi addon does not expose NativeTuiHost");
const host = new Host(80, 24, true);
const session = nativeViewAbiSession();
if (session === undefined) throw new Error("direct-ffi addon does not expose tuiViewAbiBootstrap");
const boundary = new RetainedRootBoundary(session, () => host.tuiViewAbiHostPointer());
setRootColdMaterializer(tryNativeMaterialize);
let phaseSamples: RetainedPhaseSample[] | undefined;

function render(view: View): void {
  const publication = boundary.prepareInstall(view) ?? boundary.prepareColdInstall(view);
  if (publication !== undefined) {
    publication.commit();
    return;
  }
  const fallbackStart = Bun.nanoseconds();
  host.render(nodeForBridge(view));
  if (!boundary.adopt(view)) throw new Error("direct-ffi cold fallback could not adopt root");
  const fallbackEnd = Bun.nanoseconds();
  phaseSamples?.push({ transport_prepare_ns: 0, native_materialize_ns: 0, host_commit_ns: fallbackEnd - fallbackStart });
}

try {
  const scenario = makeT15Scenario({ workload, mode, size });
  render(scenario.initial);
  for (let index = 0; index < warmup; index++) {
    render(scenario.next(index));
  }
  resetRetainedIdentityCounters();
  phaseSamples = [];
  setRetainedPhaseInstrumentation({
    now_ns: () => Bun.nanoseconds(),
    record: (sample) => phaseSamples?.push(sample),
  });
  const semanticConstruction: number[] = [];
  const transportAndHost: number[] = [];
  try {
    for (let index = 0; index < measured; index++) {
      const constructStart = Bun.nanoseconds();
      const next = scenario.next(warmup + index);
      semanticConstruction.push(Bun.nanoseconds() - constructStart);
      const renderStart = Bun.nanoseconds();
      render(next);
      transportAndHost.push(Bun.nanoseconds() - renderStart);
    }
  } finally {
    setRetainedPhaseInstrumentation(undefined);
  }
  const total = semanticConstruction.map((value, index) => value + transportAndHost[index]!);
  console.log(JSON.stringify({
    benchmark_version: "PERF-12-T15",
    profile: process.env.T15_PROFILE ?? "draft",
    candidate: process.env.T15_CANDIDATE ?? "direct_ffi_oracle",
    transport: process.env.T15_TRANSPORT ?? "feature_gated_direct_ffi",
    workload,
    size,
    mode,
    git_sha: process.env.T15_GIT_SHA ?? "unknown",
    perf7v2_sha: "e5292d62c4011610850cbdc1ba4a35f296f78e4f",
    perf11v4_result_sha: "7c670ccd99fb296b18719f62c1aa845a3e3605de",
    bun_version: Bun.version,
    bun_revision: Bun.revision,
    rustc_version: process.env.T15_RUSTC_VERSION ?? "unknown",
    target: process.env.T15_TARGET ?? "unknown",
    addon_sha256: process.env.T15_NATIVE_SHA256 ?? "unknown",
    warmup,
    measured,
    process_isolated: true,
    semantic_construction_samples_ns: semanticConstruction,
    transport_prepare_samples_ns: phaseSamples?.map((sample) => sample.transport_prepare_ns) ?? [],
    native_materialize_samples_ns: phaseSamples?.map((sample) => sample.native_materialize_ns) ?? [],
    host_commit_samples_ns: phaseSamples?.map((sample) => sample.host_commit_ns) ?? [],
    phase_visibility: "semantic_construction_plus_retained_prepare_materialize_commit",
    samples_ns: total,
    median_ns: median(total),
    p95_ns: percentile(total, 0.95),
    p99_ns: percentile(total, 0.99),
    median_ci95_ns: bootstrap(total),
    structural_delta: retainedIdentityCounterSnapshot(),
    first_screen_row: host.screenRows()[0] ?? "",
  }));
} finally {
  setRetainedPhaseInstrumentation(undefined);
  phaseSamples = undefined;
  boundary.close();
  host.dispose();
}
