import { createHash } from "node:crypto";
import { native } from "../../iyon-tui/src/native.ts";
import {
  nativeViewAbiSession,
  tryNativeAxisCreateRender,
  releaseNativeViewRef,
} from "../../iyon-tui/src/native_view_abi.ts";
import { viewSpacerCreate } from "../../iyon-tui/src/generated/view_calls.ts";
import { nodeForBridge, nodeIdPair, View } from "../../iyon-tui/src/values/view.ts";
import manifest from "../../iyon-tui/src/generated/view_abi_manifest.json";
import {
  NATIVE_BUILDER_MAX_CHILDREN,
  NATIVE_COLD_MAX_DEPTH,
  NATIVE_COLD_MAX_NODES,
  NATIVE_SMALL_AXIS_ARITY_MAX,
} from "../../iyon-tui/src/native_view_policy.ts";

const SIZES = [20, 200, 2_000, 10_000] as const;
const WARMUP = 2;
const ITERATIONS = 10;
const REPEATS = 3;
const Host = native.NativeTuiHost as unknown as (new (width: number, height: number, headless: boolean) => {
  render(view: object): void;
  tuiViewAbiHostPointer(): number;
  dispose(): void;
}) | undefined;

type Prepared = {
  readonly host: InstanceType<NonNullable<typeof Host>>;
  readonly nativeChildren: readonly View[];
  readonly directChildren: readonly View[];
  readonly nativeBuilderChildren: readonly { readonly view: View }[];
  readonly nativeChildRefs: readonly number[];
};

function median(values: readonly number[]): number {
  const sorted = [...values].sort((left, right) => left - right);
  return sorted[Math.floor(sorted.length / 2)]!;
}

function measure(prepared: Prepared, route: "builder" | "v4"): number {
  const iterations = prepared.nativeChildren.length >= 10_000
    ? 1
    : prepared.nativeChildren.length >= 2_000
      ? 3
      : ITERATIONS;
  const samples: number[] = [];
  for (let warmup = 0; warmup < WARMUP; warmup += 1) run(prepared, route);
  for (let repeat = 0; repeat < REPEATS; repeat += 1) {
    const start = Bun.nanoseconds();
    for (let iteration = 0; iteration < iterations; iteration += 1) run(prepared, route);
    samples.push(Number(Bun.nanoseconds() - start) / iterations);
  }
  return median(samples);
}

function run(prepared: Prepared, route: "builder" | "v4"): void {
  if (route === "builder") {
    const root = View.vertical(prepared.nativeChildren);
    const reference = tryNativeAxisCreateRender(
      prepared.host,
      root,
      false,
      0,
      prepared.nativeBuilderChildren,
    );
    if (reference === undefined) throw new Error("native builder route was rejected");
    releaseNativeViewRef(nativeViewAbiSession(), reference);
    return;
  }
  const root = View.vertical(prepared.directChildren);
  prepared.host.render(nodeForBridge(root));
}

function prepare(size: number, session: NonNullable<ReturnType<typeof nativeViewAbiSession>>): Prepared {
  if (Host === undefined) throw new Error("NativeTuiHost is unavailable");
  const host = new Host(80, Math.min(size + 2, 10_002), true);
  const nativeChildren = Array.from({ length: size }, (_, index) => View.spacer((index % 3) + 1));
  const nativeChildRefs = nativeChildren.map((view, index) => {
    const [low, high] = nodeIdPair(view);
    return viewSpacerCreate(session.symbols, session.runtime, low, high, (index % 3) + 1);
  });
  const directChildren = Array.from({ length: size }, (_, index) => View.spacer((index % 3) + 1));
  for (const child of directChildren) nodeForBridge(child);
  return {
    host,
    nativeChildren,
    directChildren,
    nativeBuilderChildren: nativeChildren.map((view) => ({ view })),
    nativeChildRefs,
  };
}

const session = nativeViewAbiSession();
if (session === undefined || Host === undefined) throw new Error("native View ABI is unavailable");
const results: Record<string, { builder_median_ns: number; v4_median_ns: number }> = {};
for (const size of SIZES) {
  const prepared = prepare(size, session);
  try {
    results[String(size)] = {
      builder_median_ns: measure(prepared, "builder"),
      v4_median_ns: measure(prepared, "v4"),
    };
  } finally {
    for (const reference of prepared.nativeChildRefs) releaseNativeViewRef(session, reference);
    prepared.host.dispose();
  }
}

const nativeArtifact = await Bun.file(new URL("../../../iyon-tui/native/iyon-tui-native.node", import.meta.url)).arrayBuffer();
const nativeArtifactSha256 = createHash("sha256").update(new Uint8Array(nativeArtifact)).digest("hex");
const git = (command: string): string => {
  const result = Bun.spawnSync(["git", ...command.split(" ")], { stdout: "pipe", stderr: "ignore" });
  return new TextDecoder().decode(result.stdout).trim();
};
console.log(JSON.stringify({
  benchmark: "PERF-11.8-native-builders",
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
  sizes: SIZES,
  warmup: WARMUP,
  iterations: ITERATIONS,
  repeats: REPEATS,
  iteration_policy: "10 for <=200, 3 for 2,000, 1 for 10,000",
  routing_thresholds: {
    small_arity_max: NATIVE_SMALL_AXIS_ARITY_MAX,
    builder_max_children: NATIVE_BUILDER_MAX_CHILDREN,
    cold_max_nodes: NATIVE_COLD_MAX_NODES,
    cold_max_depth: NATIVE_COLD_MAX_DEPTH,
  },
  routes: {
    builder: "compact pending axis -> generated small arity/builder -> one host install",
    v4: "pre-materialized direct bridge -> existing host decoder",
  },
  results,
  structural_encoding_ns: 0,
  command_words_written: 0,
  path_arrays_written: 0,
  node_id_arrays_written: 0,
}, null, 2));
