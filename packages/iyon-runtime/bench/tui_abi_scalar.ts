import { createHash } from "node:crypto";
import type { Pointer } from "bun:ffi";
import { native } from "../../iyon-tui/src/native.ts";
import { nativeViewAbiSession } from "../../iyon-tui/src/native_view_abi.ts";
import {
  hostRenderRef,
  viewRefForNodeId,
  viewReleaseMany,
  viewRenderRef,
  viewTextLayoutPatchRoot,
} from "../../iyon-tui/src/generated/view_calls.ts";
import manifest from "../../iyon-tui/src/generated/view_abi_manifest.json";
import { nodeForBridge, nodeIdPair, View } from "../../iyon-tui/src/values/view.ts";

const HOT_ITERATIONS = 1_000_000;
const ALLOCATION_ITERATIONS = 10_000;
const REPEATS = 5;
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

function measure(iterations: number, call: (index: number) => number): Timing {
  const samples: number[] = [];
  let checksum = 0;
  for (let repeat = 0; repeat < REPEATS; repeat += 1) {
    const start = Bun.nanoseconds();
    for (let index = 0; index < iterations; index += 1) checksum ^= call(index);
    samples.push(Number(Bun.nanoseconds() - start) / iterations);
  }
  return { median_ns_per_call: median(samples), checksum };
}

function git(command: string): string {
  const result = Bun.spawnSync(["git", ...command.split(" ")], { stdout: "pipe", stderr: "ignore" });
  return new TextDecoder().decode(result.stdout).trim();
}

const session = nativeViewAbiSession();
if (session === undefined) throw new Error("generated native View ABI is unavailable");
if (Host === undefined) throw new Error("NativeTuiHost is unavailable");

const host = new Host(40, 8, true);
const hostPointer = host.tuiViewAbiHostPointer() as unknown as Pointer;
const base = View.text("PERF-11");
host.render(nodeForBridge(base));
const [baseLow, baseHigh] = nodeIdPair(base);
const baseRef = viewRefForNodeId(session.symbols, session.runtime, baseLow, baseHigh);
const patchRefs = new Uint32Array(ALLOCATION_ITERATIONS);

for (let index = 0; index < 100_000; index += 1) {
  viewRenderRef(session.symbols, session.runtime, baseRef);
  hostRenderRef(session.symbols, session.runtime, hostPointer, baseRef);
}

const results = {
  runtime_noop: measure(HOT_ITERATIONS, () => session.symbols.runtimeNoop(session.runtime)),
  view_render_ref: measure(HOT_ITERATIONS, () => viewRenderRef(session.symbols, session.runtime, baseRef)),
  host_render_ref: measure(HOT_ITERATIONS, () => hostRenderRef(session.symbols, session.runtime, hostPointer, baseRef)),
  view_text_layout_patch_root: measure(ALLOCATION_ITERATIONS, (index) => {
    const reference = viewTextLayoutPatchRoot(
      session.symbols,
      session.runtime,
      baseRef,
      index + 100_000,
      0,
      index % 2 === 0 ? 2 : 3,
      index % 3 === 0 ? 2 : 1,
    );
    patchRefs[index] = reference;
    return reference;
  }),
};

viewReleaseMany(session.symbols, session.runtime, patchRefs, ALLOCATION_ITERATIONS);
viewReleaseMany(session.symbols, session.runtime, new Uint32Array([baseRef]), 1);
host.dispose();

const nativeArtifact = await Bun.file(new URL("../../../iyon-tui/native/iyon-tui-native.node", import.meta.url)).arrayBuffer();
const nativeArtifactSha256 = createHash("sha256").update(new Uint8Array(nativeArtifact)).digest("hex");
console.log(JSON.stringify({
  benchmark: "PERF-11.3-generated-scalar",
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
  fast_view_abi: session.abi.fast_view_abi,
  hot_iterations: HOT_ITERATIONS,
  allocation_iterations: ALLOCATION_ITERATIONS,
  repeats: REPEATS,
  structural_encoding_ns: 0,
  command_words_written: 0,
  path_arrays_written: 0,
  node_id_arrays_written: 0,
  results,
}, null, 2));
