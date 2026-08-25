import { createHash } from "node:crypto";
import type { Pointer } from "bun:ffi";
import { native } from "../../iyon-tui/src/native.ts";
import {
  hostRenderRef,
  viewCommonPatchRoot,
  viewRefForNodeId,
} from "../../iyon-tui/src/generated/view_calls.ts";
import { nativeViewAbiSession, releaseNativeViewRef, tryNativeScalarRender } from "../../iyon-tui/src/native_view_abi.ts";
import { nodeForBridge, nodeIdPair, viewBackingState, View } from "../../iyon-tui/src/values/view.ts";
import manifest from "../../iyon-tui/src/generated/view_abi_manifest.json";

const ITERATIONS = 10_000;
const REPEATS = 5;
const COMMON_MASK = 4 | 64 | 128;
const Host = native.NativeTuiHost as unknown as (new (width: number, height: number, headless: boolean) => {
  render(view: object): void;
  tuiViewAbiHostPointer(): number;
  dispose(): void;
}) | undefined;

type Timing = { median_ns_per_call: number; checksum: number };

function median(values: readonly number[]): number {
  const sorted = [...values].sort((left, right) => left - right);
  return sorted[Math.floor(sorted.length / 2)]!;
}

function measure(call: (index: number) => number): Timing {
  const samples: number[] = [];
  let checksum = 0;
  for (let repeat = 0; repeat < REPEATS; repeat += 1) {
    const start = Bun.nanoseconds();
    for (let index = 0; index < ITERATIONS; index += 1) checksum ^= call(index);
    samples.push(Number(Bun.nanoseconds() - start) / ITERATIONS);
  }
  return { median_ns_per_call: median(samples), checksum };
}

function git(command: string): string {
  const result = Bun.spawnSync(["git", ...command.split(" ")], { stdout: "pipe", stderr: "ignore" });
  return new TextDecoder().decode(result.stdout).trim();
}

function lazyFusedView(base: View = View.text("status")): View {
  return base
    .padding(1)
    .maxWidth(40)
    .minHeight(1);
}

function forcedBridgeView(): View {
  let view = View.text("status");
  nodeForBridge(view);
  view = view.padding(1);
  nodeForBridge(view);
  view = view.maxWidth(40);
  nodeForBridge(view);
  view = view.minHeight(1);
  nodeForBridge(view);
  return view;
}

const session = nativeViewAbiSession();
if (session === undefined) throw new Error("generated native View ABI is unavailable");
if (Host === undefined) throw new Error("NativeTuiHost is unavailable");

const host = new Host(40, 8, true);
const hostPointer = host.tuiViewAbiHostPointer() as unknown as Pointer;
const base = View.text("status");
host.render(nodeForBridge(base));
const [baseLow, baseHigh] = nodeIdPair(base);
const baseRef = viewRefForNodeId(session.symbols, session.runtime, baseLow, baseHigh);

const results = {
  forced_bridge_construction: measure((index) => {
    const view = forcedBridgeView();
    return viewNodeChecksum(view, index);
  }),
  lazy_fused_construction: measure((index) => {
    const view = lazyFusedView();
    if (viewBackingState(view) !== 2) throw new Error("lazy fused view did not remain pending");
    return viewNodeChecksum(view, index);
  }),
  lazy_fused_native_retained: measure((index) => {
    const view = lazyFusedView(base);
    const reference = tryNativeScalarRender(host, base, baseRef, view);
    if (reference === undefined) throw new Error("lazy fused scalar route unexpectedly fell back");
    releaseNativeViewRef(session, reference);
    return reference;
  }),
  immediate_native_retained: measure((index) => {
    const reference = viewCommonPatchRoot(
      session.symbols,
      session.runtime,
      baseRef,
      index + 100_000,
      0,
      COMMON_MASK,
      1,
      1,
      0,
      0,
      0,
      40,
      1,
      0,
      baseRef,
    );
    const status = hostRenderRef(session.symbols, session.runtime, hostPointer, reference);
    if (status !== 0) throw new Error(`immediate native host install failed: ${status}`);
    releaseNativeViewRef(session, reference);
    return reference;
  }),
};

releaseNativeViewRef(session, baseRef);
host.dispose();

const nativeArtifact = await Bun.file(new URL("../../../iyon-tui/native/iyon-tui-native.node", import.meta.url)).arrayBuffer();
const nativeArtifactSha256 = createHash("sha256").update(new Uint8Array(nativeArtifact)).digest("hex");
console.log(JSON.stringify({
  benchmark: "PERF-11.5-native-construction",
  bun_version: Bun.version,
  bun_revision: Bun.revision,
  git_sha: git("rev-parse HEAD"),
  git_dirty: git("status --porcelain") !== "",
  native_artifact_sha256: nativeArtifactSha256,
  schema_blake3: manifest.schema_blake3,
  generator_blake3: manifest.generator_blake3,
  abi_version: session.abi.abi_version,
  semantic_schema_version: session.abi.semantic_version,
  function_count: session.abi.function_count,
  iterations: ITERATIONS,
  repeats: REPEATS,
  fluent_steps: 3,
  forced_bridge_materializations: 4,
  lazy_fused_materializations_before_native: 0,
  structural_encoding_ns: 0,
  command_words_written: 0,
  path_arrays_written: 0,
  node_id_arrays_written: 0,
  results,
}, null, 2));

function viewNodeChecksum(view: View, index: number): number {
  const [low, high] = nodeIdPair(view);
  return (low ^ high ^ index) >>> 0;
}
