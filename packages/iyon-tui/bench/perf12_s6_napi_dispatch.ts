import { native } from "../src/native.ts";

const session = native.tuiViewAbiSession?.();
const gitSha = process.env.PERF12_GIT_SHA ?? "unknown";
const nativeArtifactSha256 = process.env.PERF12_NATIVE_SHA256 ?? "unknown";
const rustcVersion = process.env.PERF12_RUSTC_VERSION ?? "unknown";
const target = process.env.PERF12_TARGET ?? "unknown";
if (session === undefined || session.tuiPerfNapiBatchRuntimeNoop === undefined) {
  throw new Error("S6 N-API dispatch probe is unavailable");
}
const count = 10_000;
const warmup = 0;
for (let i = 0; i < warmup; i++) {
  for (let j = 0; j < count; j++) session.runtimeNoop();
  session.tuiPerfNapiBatchRuntimeNoop(count);
}

function median(values: readonly number[]): number {
  const ordered = [...values].sort((left, right) => left - right);
  return ordered[Math.floor(ordered.length / 2)]!;
}

const perCall: number[] = [];
const batched: number[] = [];
for (let round = 0; round < 20; round++) {
  let start = Bun.nanoseconds();
  for (let index = 0; index < count; index++) session.runtimeNoop();
  perCall.push((Bun.nanoseconds() - start) / count);
  start = Bun.nanoseconds();
  session.tuiPerfNapiBatchRuntimeNoop(count);
  batched.push((Bun.nanoseconds() - start) / count);
}

console.log(JSON.stringify({
  benchmark_version: "PERF-12-S6-dispatch",
  profile: "smoke",
  candidate: "napi",
  git_sha: gitSha,
  bun_version: Bun.version,
  bun_revision: Bun.revision,
  rustc_version: rustcVersion,
  target,
  native_artifact_sha256: nativeArtifactSha256,
  count,
  rounds: perCall.length,
  per_call_median_ns: median(perCall),
  batched_median_ns: median(batched),
  per_call_samples_ns: perCall,
  batched_samples_ns: batched,
}));
