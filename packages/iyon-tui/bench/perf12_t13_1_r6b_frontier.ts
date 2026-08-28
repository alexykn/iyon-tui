import { Scene, View } from "../src/index.ts";
import { AppHarness } from "../src/testing/index.ts";

const nativePerf = process.env.T13_R6B_COUNTERS === "1"
  ? require("../native/iyon-tui-native.node") as {
    tuiPerfReset?: () => void;
    tuiPerfSnapshot?: () => Record<string, number>;
  }
  : undefined;
const scopeCount = Number(process.env.T13_R6B_SCOPES ?? 1_000);
const workload = process.env.T13_R6B_WORKLOAD ?? "same_geometry";
const warmup = 50;
const measured = 200;

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
    const sample = Array.from(
      { length: values.length },
      () => values[Math.floor(Math.random() * values.length)]!,
    );
    medians.push(median(sample));
  }
  medians.sort((left, right) => left - right);
  return [medians[Math.floor(medians.length * 0.025)]!, medians[Math.floor(medians.length * 0.975)]!];
}

const tui = await AppHarness.open({ width: 80, height: 24 });
try {
  const slots = Array.from({ length: scopeCount }, () => tui.createViewSlot(View.text("x")));
  const body = View.vertical(slots.map((slot) => slot.view()));
  await tui.render(new Scene(body));

  const update = (index: number): void => {
    const value = workload === "same_geometry" ? `x${index % 2}` : `x${index}\nrow`;
    slots[0]!.setView(View.text(value));
  };
  for (let index = 0; index < warmup; index++) update(index);

  nativePerf?.tuiPerfReset?.();
  const samples: number[] = [];
  for (let index = 0; index < measured; index++) {
    const start = Bun.nanoseconds();
    update(warmup + index);
    samples.push(Bun.nanoseconds() - start);
  }

  console.log(JSON.stringify({
    benchmark_version: "PERF-12-T13.1-R6b",
    profile: "smoke",
    candidate: "napi_default",
    transport: "generated_safe_napi",
    workload,
    scope_count: scopeCount,
    git_sha: process.env.T13_R6B_GIT_SHA ?? "unknown",
    bun_version: Bun.version,
    bun_revision: Bun.revision,
    rustc_version: process.env.T13_R6B_RUSTC_VERSION ?? "unknown",
    target: process.env.T13_R6B_TARGET ?? "unknown",
    native_artifact_sha256: process.env.T13_R6B_NATIVE_SHA256 ?? "unknown",
    warmup,
    measured,
    process_isolated: process.env.T13_R6B_WORKLOAD !== undefined,
    samples_ns: samples,
    median_ns: median(samples),
    p95_ns: percentile(samples, 0.95),
    p99_ns: percentile(samples, 0.99),
    median_ci95_ns: bootstrapMedianInterval(samples),
    first_screen_row: tui.screenRows()[0] ?? "",
    counters: nativePerf?.tuiPerfSnapshot?.(),
  }));
} finally {
  await tui.close();
}
