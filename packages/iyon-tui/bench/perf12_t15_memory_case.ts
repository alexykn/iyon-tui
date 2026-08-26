import { View } from "../src/index.ts";
import { nodeForBridge } from "../src/view-internals.ts";

const transport = process.env.T15_TRANSPORT ?? "generated_safe_napi";
const direct = transport === "feature_gated_direct_ffi";
const operations = Number(process.env.T15_MEMORY_OPS ?? 1_000_000);
const interval = Number(process.env.T15_MEMORY_INTERVAL ?? 100_000);
if (!Number.isSafeInteger(operations) || operations <= 0 || !Number.isSafeInteger(interval) || interval <= 0) {
  throw new Error("invalid memory benchmark operation counts");
}

interface NativeCaseModule {
  readonly native: {
    readonly NativeTuiHost?: new (width?: number, height?: number, headless?: boolean) => {
      render(view: object): void;
      screenRows(): string[];
      dispose(): void;
      tuiViewAbiHostPointer?(): number;
      [key: string]: unknown;
    };
    readonly tuiViewAbiMaintain?: (full?: boolean) => unknown;
    readonly tuiViewRuntimeMemorySnapshot?: (countLive?: boolean) => unknown;
  };
}

interface AbiCaseModule {
  nativeViewAbiSession(): unknown;
  tryNativeMaterialize(view: View): number | undefined;
}

interface RetainedCaseModule {
  readonly RetainedRootBoundary: new (session: unknown, host: () => unknown) => {
    prepareInstall(view: View): { commit(): void } | undefined;
    prepareColdInstall(view: View): { commit(): void } | undefined;
    adopt(view: View): boolean;
    close(): void;
  };
  setRootColdMaterializer(materializer: ((view: View) => number | undefined) | undefined): void;
}

const nativeModule = (direct
  ? await import("./direct_ffi/native.ts")
  : await import("../src/native.ts")) as unknown as NativeCaseModule;
const abi = (direct
  ? await import("./direct_ffi/native_view_abi.ts")
  : await import("../src/native_view_abi.ts")) as unknown as AbiCaseModule;
const retained = (direct
  ? await import("./direct_ffi/retained_dag.ts")
  : await import("../src/retained_dag.ts")) as unknown as RetainedCaseModule;

const Host = nativeModule.native.NativeTuiHost;
if (Host === undefined) throw new Error(`missing NativeTuiHost for ${transport}`);
const host = new Host(80, 24, true);
const session = abi.nativeViewAbiSession();
if (session === undefined) throw new Error(`missing native ABI session for ${transport}`);
const hostTarget = direct
  ? () => host.tuiViewAbiHostPointer?.()
  : () => host;
const boundary = new retained.RetainedRootBoundary(session, hostTarget);
retained.setRootColdMaterializer(abi.tryNativeMaterialize);

function render(view: View): void {
  const publication = boundary.prepareInstall(view) ?? boundary.prepareColdInstall(view);
  if (publication !== undefined) {
    publication.commit();
    return;
  }
  host.render(nodeForBridge(view));
  if (!boundary.adopt(view)) throw new Error("memory case failed to adopt root");
}

const stable = View.vertical(
  Array.from({ length: 200 }, (_, index) => View.text(`stable-${index}`).noWrap()),
);
let root = View.vertical([View.text("changed-0").noWrap(), stable]);
const retainedViews: View[] = [];
const samples: Array<Record<string, unknown>> = [];

try {
  render(root);
  for (let operation = 1; operation <= operations; operation++) {
    const changed = View.text(`changed-${operation}`).noWrap();
    root = View.vertical([changed, stable]);
    render(root);
    if (operation % 10_000 === 0) retainedViews.push(root);

    if (operation % interval === 0) {
      if (typeof Bun.gc === "function") Bun.gc(true);
      nativeModule.native.tuiViewAbiMaintain?.(true);
      samples.push({
        operation,
        rss_bytes: process.memoryUsage().rss,
        snapshot: nativeModule.native.tuiViewRuntimeMemorySnapshot?.(true) ?? null,
      });
    }
  }

  console.log(JSON.stringify({
    benchmark_version: "PERF-12-T15-MEMORY",
    profile: process.env.T15_PROFILE ?? "authoritative",
    candidate: direct ? "direct_ffi_oracle" : "napi_default",
    transport,
    git_sha: process.env.T15_GIT_SHA ?? "unknown",
    operations,
    interval,
    retained_view_count: retainedViews.length,
    samples,
  }));
} finally {
  boundary.close();
  retained.setRootColdMaterializer(undefined);
  host.dispose();
}
