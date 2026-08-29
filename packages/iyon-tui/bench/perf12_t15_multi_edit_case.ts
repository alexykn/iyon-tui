import { View } from "../src/index.ts";
import { nativePathChildLineage, NATIVE_PATH_STEP, NATIVE_PATH_VIEW_KIND } from "../src/transport/structural/retained-path.ts";
import { viewNodeId, type View as ViewValue } from "../src/api/view/view.ts";
import { lowerColdView } from "../src/transport/structural/cold-lowering.ts";

const transport = process.env.T15_TRANSPORT ?? "generated_safe_napi";
const direct = transport === "feature_gated_direct_ffi";
const editCount = Number(process.env.T15_EDIT_COUNT ?? 2);
const warmup = Number(process.env.T15_WARMUP ?? 50);
const measured = Number(process.env.T15_MEASURED ?? 1_000);
if (![2, 8, 32, 64].includes(editCount)) throw new Error(`unsupported edit count ${editCount}`);

interface HostContract {
  render(view: object): void;
  screenRows(): string[];
  dispose(): void;
  tuiViewAbiHostPointer?(): number;
  [key: string]: unknown;
}

interface AbiModule {
  nativeViewAbiSession(): unknown;
  nativeViewRefForNodeId(view: ViewValue): number | undefined;
  releaseNativeViewRef(session: unknown, ref: number): void;
  tryNativeEditTransactionRender(
    host: unknown,
    previous: ViewValue,
    previousRef: number,
    edits: readonly unknown[],
  ): number | undefined;
}

interface NativeModule {
  readonly native: {
    readonly NativeTuiHost?: new (width?: number, height?: number, headless?: boolean) => HostContract;
  };
}

const nativeModule = (direct
  ? await import("./direct_ffi/native.ts")
  : await import("../src/transport/native/addon.ts")) as unknown as NativeModule;
const abi = (direct
  ? await import("./direct_ffi/native_view_abi.ts")
  : await import("../src/transport/structural/native-view-abi.ts")) as unknown as AbiModule;
const Host = nativeModule.native.NativeTuiHost;
if (Host === undefined) throw new Error(`missing NativeTuiHost for ${transport}`);
const host = new Host(80, Math.max(8, editCount + 2), true);
const session = abi.nativeViewAbiSession();
if (session === undefined) throw new Error(`missing native ABI session for ${transport}`);

function median(values: readonly number[]): number {
  const ordered = [...values].sort((left, right) => left - right);
  return ordered[Math.floor(ordered.length / 2)]!;
}

function percentile(values: readonly number[], fraction: number): number {
  const ordered = [...values].sort((left, right) => left - right);
  return ordered[Math.min(ordered.length - 1, Math.ceil(ordered.length * fraction) - 1)]!;
}

function bootstrap(values: readonly number[], rounds = 500): [number, number] {
  const medians: number[] = [];
  for (let round = 0; round < rounds; round++) {
    const sample = Array.from({ length: values.length }, () => values[Math.floor(Math.random() * values.length)]!);
    medians.push(median(sample));
  }
  medians.sort((left, right) => left - right);
  return [medians[Math.floor(rounds * 0.025)]!, medians[Math.floor(rounds * 0.975)]!];
}

const baseChildren = Array.from({ length: editCount }, (_, index) => View.text(`base-${index}`).noWrap());
const base = View.vertical(baseChildren);
host.render(lowerColdView(base));
const baseRef = abi.nativeViewRefForNodeId(base);
if (baseRef === undefined) throw new Error("unable to acquire base NativeRef");

function makeChanged(seed: number): { root: View; edits: readonly unknown[] } {
  const changedChildren = baseChildren.map((child, index) => child.textAlign((seed + index) % 2 === 0 ? "center" : "end"));
  const root = View.vertical(changedChildren);
  const edits = changedChildren.map((child, index) => ({
    lineage: nativePathChildLineage(base, undefined, {
      kind: NATIVE_PATH_STEP.columnChild,
      expectedViewKind: NATIVE_PATH_VIEW_KIND.column,
      selector: index,
    }),
    nodeIds: [viewNodeId(child), viewNodeId(root)],
    wrap: (seed + index) % 2 === 0 ? 3 : 2,
    align: (seed + index) % 2 === 0 ? 1 : 2,
  }));
  return { root, edits };
}

function renderTransaction(seed: number): void {
  const next = makeChanged(seed);
  const result = abi.tryNativeEditTransactionRender(host, base, baseRef!, next.edits);
  if (result === undefined) throw new Error(`edit transaction refused at seed ${seed}`);
  abi.releaseNativeViewRef(session, result);
}

try {
  for (let index = 0; index < warmup; index++) renderTransaction(index);
  const semanticConstruction: number[] = [];
  const samples: number[] = [];
  for (let index = 0; index < measured; index++) {
    const constructStart = Bun.nanoseconds();
    const next = makeChanged(warmup + index);
    semanticConstruction.push(Bun.nanoseconds() - constructStart);
    const start = Bun.nanoseconds();
    const result = abi.tryNativeEditTransactionRender(host, base, baseRef, next.edits);
    if (result === undefined) throw new Error(`edit transaction refused at measured seed ${index}`);
    samples.push(Bun.nanoseconds() - start);
    abi.releaseNativeViewRef(session, result);
  }
  const final = makeChanged(warmup + measured + 1);
  const finalRef = abi.tryNativeEditTransactionRender(host, base, baseRef, final.edits);
  if (finalRef === undefined) throw new Error("final edit transaction refused");
  const oracle = new Host(80, Math.max(8, editCount + 2), true);
  try {
    oracle.render(lowerColdView(final.root));
    const correctness = JSON.stringify(host.screenRows()) === JSON.stringify(oracle.screenRows());
    console.log(JSON.stringify({
      benchmark_version: "PERF-12-T15-MULTI-EDIT",
      profile: process.env.T15_PROFILE ?? "authoritative",
      candidate: direct ? "direct_ffi_oracle" : "napi_default",
      transport,
      workload: "multi_edit",
      mode: `edit_txn_${editCount}`,
      size: editCount,
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
      samples_ns: samples,
      median_ns: median(samples),
      p95_ns: percentile(samples, 0.95),
      p99_ns: percentile(samples, 0.99),
      median_ci95_ns: bootstrap(samples),
      correctness,
      transport_prepare_samples_ns: [],
      native_materialize_samples_ns: [],
      host_commit_samples_ns: [],
    }));
  } finally {
    oracle.dispose();
  }
  abi.releaseNativeViewRef(session, finalRef);
} finally {
  abi.releaseNativeViewRef(session, baseRef);
  host.dispose();
}
