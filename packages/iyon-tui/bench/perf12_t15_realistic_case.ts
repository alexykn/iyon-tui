import { View } from "../src/index.ts";
import { nodeForBridge } from "../src/transport/structural/view-bridge.ts";
import type { RetainedPhaseSample } from "../src/transport/structural/retained-dag.ts";

const transport = process.env.T15_TRANSPORT ?? "generated_safe_napi";
const direct = transport === "feature_gated_direct_ffi";
const warmup = Number(process.env.T15_WARMUP ?? 50);
const measured = Number(process.env.T15_MEASURED ?? 1_000);

interface HostContract {
  render(view: object): void;
  screenRows(): string[];
  dispose(): void;
  tuiViewAbiHostPointer?(): number;
  [key: string]: unknown;
}

interface NativeModule {
  readonly native: {
    readonly NativeTuiHost?: new (width?: number, height?: number, headless?: boolean) => HostContract;
  };
}

interface AbiModule {
  nativeViewAbiSession(): unknown;
  tryNativeMaterialize(view: View): number | undefined;
}

interface RetainedModule {
  readonly RetainedRootBoundary: new (session: unknown, host: () => unknown) => {
    prepareInstall(view: View): { commit(): void } | undefined;
    prepareColdInstall(view: View): { commit(): void } | undefined;
    adopt(view: View): boolean;
    close(): void;
  };
  setRootColdMaterializer(materializer: ((view: View) => number | undefined) | undefined): void;
  setRetainedPhaseInstrumentation(instrumentation: {
    readonly now_ns: () => number;
    readonly record: (sample: RetainedPhaseSample) => void;
  } | undefined): void;
}

const nativeModule = (direct
  ? await import("./direct_ffi/native.ts")
  : await import("../src/transport/native/addon.ts")) as unknown as NativeModule;
const abi = (direct
  ? await import("./direct_ffi/native_view_abi.ts")
  : await import("../src/transport/structural/native-view-abi.ts")) as unknown as AbiModule;
const retained = (direct
  ? await import("./direct_ffi/retained_dag.ts")
  : await import("../src/transport/structural/retained-dag.ts")) as unknown as RetainedModule;
const Host = nativeModule.native.NativeTuiHost;
if (Host === undefined) throw new Error(`missing NativeTuiHost for ${transport}`);
const host = new Host(80, 24, true);
const session = abi.nativeViewAbiSession();
if (session === undefined) throw new Error(`missing native ABI session for ${transport}`);
const boundary = new retained.RetainedRootBoundary(
  session,
  direct ? () => host.tuiViewAbiHostPointer?.() : () => host,
);
retained.setRootColdMaterializer(abi.tryNativeMaterialize);

const shellHeader = View.text("shell").bold().noWrap();
const historyRows = Array.from({ length: 24 }, (_, index) => View.text(`history-${index}`).noWrap());
let stream = View.text("stream-0").noWrap();
let status = View.text("status-idle").padding(1);
let footer = View.text("footer").noWrap();
let history = historyRows;

function frame(step: number): View {
  if (step % 17 === 0) {
    history = [...history, View.text(`history-${step}`).noWrap()];
    if (history.length > 200) history = history.slice(history.length - 200);
  }
  if (step % 3 === 0) stream = View.text(`stream-${step}\nchunk-${step}`).noWrap();
  if (step % 11 === 0) status = View.text(`status-${step}`).padding(step % 22 === 0 ? 1 : 2);
  if (step % 29 === 0) footer = View.text(`footer-${step}`).noWrap();
  return View.vertical([
    shellHeader,
    View.vertical(history),
    stream,
    status,
    footer,
  ]);
}

let phaseSamples: RetainedPhaseSample[] | undefined;
function render(view: View): void {
  const publication = boundary.prepareInstall(view) ?? boundary.prepareColdInstall(view);
  if (publication !== undefined) {
    publication.commit();
    return;
  }
  const fallbackStart = Bun.nanoseconds();
  host.render(nodeForBridge(view));
  if (!boundary.adopt(view)) throw new Error("realistic trace fallback could not adopt root");
  const fallbackEnd = Bun.nanoseconds();
  phaseSamples?.push({ transport_prepare_ns: 0, native_materialize_ns: 0, host_commit_ns: fallbackEnd - fallbackStart });
}

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

try {
  render(frame(0));
  for (let index = 0; index < warmup; index++) render(frame(index + 1));
  const semanticConstruction: number[] = [];
  const samples: number[] = [];
  phaseSamples = [];
  retained.setRetainedPhaseInstrumentation({
    now_ns: () => Bun.nanoseconds(),
    record: (sample) => phaseSamples?.push(sample),
  });
  try {
    for (let index = 0; index < measured; index++) {
      const constructStart = Bun.nanoseconds();
      const next = frame(warmup + index + 1);
      semanticConstruction.push(Bun.nanoseconds() - constructStart);
      const start = Bun.nanoseconds();
      render(next);
      samples.push(Bun.nanoseconds() - start);
    }
  } finally {
    retained.setRetainedPhaseInstrumentation(undefined);
  }
  console.log(JSON.stringify({
    benchmark_version: "PERF-12-T15-REALISTIC",
    profile: process.env.T15_PROFILE ?? "authoritative",
    candidate: direct ? "direct_ffi_oracle" : "napi_default",
    transport,
    workload: "generic_terminal_trace",
    mode: "history_stream_status_layout_structural",
    size: history.length,
    git_sha: process.env.T15_GIT_SHA ?? "unknown",
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
    samples_ns: samples,
    median_ns: median(samples),
    p95_ns: percentile(samples, 0.95),
    p99_ns: percentile(samples, 0.99),
    median_ci95_ns: bootstrap(samples),
    first_screen_row: host.screenRows()[0] ?? "",
  }));
} finally {
  retained.setRetainedPhaseInstrumentation(undefined);
  phaseSamples = undefined;
  boundary.close();
  retained.setRootColdMaterializer(undefined);
  host.dispose();
}
