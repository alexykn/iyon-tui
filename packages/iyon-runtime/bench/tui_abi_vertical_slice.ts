import { createHash } from "node:crypto";
import { native } from "../src/native.ts";
import { nativeViewAbiSession } from "../src/tui/native_view_abi.ts";
import {
  runtimeNoop,
  viewCommonPatchRoot,
  viewRefForNodeId,
  viewReleaseMany,
  viewRenderRef,
  viewSpacerCreate,
  viewTextLayoutPatchRoot,
} from "../src/tui/generated/view_calls.ts";
import manifest from "../src/tui/generated/view_abi_manifest.json";
import { nodeForBridge, nodeIdPair, View } from "../src/tui/values/view.ts";

const ITERATIONS = 1_000_000;
const ALLOCATION_ITERATIONS = 10_000;
const REPEATS = 5;
const Host = native.NativeTuiHost as unknown as (new (width: number, height: number, headless: boolean) => {
  render(view: object): void;
  dispose(): void;
}) | undefined;

function median(values: readonly number[]): number {
  const sorted = [...values].sort((left, right) => left - right);
  return sorted[Math.floor(sorted.length / 2)]!;
}

function measure(iterations: number, call: (index: number) => number): { median_ns_per_call: number; checksum: number } {
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

for (let index = 0; index < 100_000; index += 1) runtimeNoop(session.symbols, session.runtime);

const host = new Host(40, 8, true);
const text = View.text("PERF-11");
host.render(nodeForBridge(text));
const [textLow, textHigh] = nodeIdPair(text);
const textRef = viewRefForNodeId(session.symbols, session.runtime, textLow, textHigh);
const releaseRefs = new Uint32Array(ALLOCATION_ITERATIONS);
const textPatchRefs = new Uint32Array(ALLOCATION_ITERATIONS);
const commonPatchRefs = new Uint32Array(ALLOCATION_ITERATIONS);
const packedPadding = (1 << 16) | 1;

const results = {
  runtime_noop: measure(ITERATIONS, () => runtimeNoop(session.symbols, session.runtime)),
  render_ref: measure(ITERATIONS, () => viewRenderRef(session.symbols, session.runtime, textRef)),
  ref_for_node_id: measure(ITERATIONS, () => viewRefForNodeId(session.symbols, session.runtime, textLow, textHigh)),
  spacer_create: measure(ALLOCATION_ITERATIONS, (index) => {
    const reference = viewSpacerCreate(session.symbols, session.runtime, index + 100_000, 0, 1);
    releaseRefs[index] = reference;
    return reference;
  }),
  text_layout_patch_root: measure(ALLOCATION_ITERATIONS, (index) => {
    const reference = viewTextLayoutPatchRoot(
      session.symbols,
      session.runtime,
      textRef,
      index + 200_000,
      0,
      index % 2 === 0 ? 2 : 3,
      index % 3 === 0 ? 2 : 1,
    );
    textPatchRefs[index] = reference;
    return reference;
  }),
  common_patch_root: measure(ALLOCATION_ITERATIONS, (index) => {
    const reference = viewCommonPatchRoot(
      session.symbols,
      session.runtime,
      textRef,
      index + 300_000,
      0,
      4,
      packedPadding,
      packedPadding,
      1,
      1,
      0,
      40,
      0,
      8,
      textRef,
    );
    commonPatchRefs[index] = reference;
    return reference;
  }),
};

viewReleaseMany(session.symbols, session.runtime, releaseRefs, ALLOCATION_ITERATIONS);
viewReleaseMany(session.symbols, session.runtime, textPatchRefs, ALLOCATION_ITERATIONS);
viewReleaseMany(session.symbols, session.runtime, commonPatchRefs, ALLOCATION_ITERATIONS);
viewReleaseMany(session.symbols, session.runtime, new Uint32Array([textRef]), 1);
host.dispose();

const nativeArtifact = await Bun.file(new URL("../native/iyon-tui-native.node", import.meta.url)).arrayBuffer();
const nativeArtifactSha256 = createHash("sha256").update(new Uint8Array(nativeArtifact)).digest("hex");
const output = {
  benchmark: "PERF-11.1-generated-vertical-slice",
  bun_version: Bun.version,
  bun_revision: Bun.revision,
  git_sha: git("rev-parse HEAD"),
  git_dirty: git("status --porcelain") !== "",
  native_artifact_sha256: nativeArtifactSha256,
  schema_blake3: manifest.schema_blake3,
  generator_blake3: manifest.generator_blake3,
  abi_version: session.abi.abi_version,
  semantic_schema_version: session.abi.semantic_version,
  fast_view_abi: session.abi.fast_view_abi,
  function_count: session.abi.function_count,
  iterations: ITERATIONS,
  allocation_iterations: ALLOCATION_ITERATIONS,
  repeats: REPEATS,
  structural_encoding_ns: 0,
  command_words_written: 0,
  path_arrays_written: 0,
  node_id_arrays_written: 0,
  results,
};
console.log(JSON.stringify(output, null, 2));
