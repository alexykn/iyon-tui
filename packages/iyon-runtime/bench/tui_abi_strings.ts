import { createHash } from "node:crypto";
import { native } from "../../iyon-tui/src/native.ts";
import { nativeViewAbiSession, tryNativeTextCreateRender, releaseNativeViewRef } from "../../iyon-tui/src/native_view_abi.ts";
import { nodeForBridge, View } from "../../iyon-tui/src/values/view.ts";
import { TextSpan } from "../../iyon-tui/src/values/text.ts";
import { StyleSpec } from "../../iyon-tui/src/values/style.ts";
import manifest from "../../iyon-tui/src/generated/view_abi_manifest.json";

const WARMUP = 2;
const ITERATIONS = 1_000;
const REPEATS = 3;
const Host = native.NativeTuiHost as unknown as (new (width: number, height: number, headless: boolean) => {
  render(view: object): void;
  tuiViewAbiHostPointer(): number;
  dispose(): void;
}) | undefined;

type Case = {
  readonly name: string;
  readonly make: () => View;
};

const cases: readonly Case[] = [
  { name: "short_ascii", make: () => View.text("status") },
  { name: "short_unicode", make: () => View.text("héllo 🌍") },
  { name: "embedded_nul", make: () => View.text("left\0right") },
  { name: "embedded_nul_spans_2", make: () => View.styledText([TextSpan.plain("left\0"), TextSpan.styled("right\0✓", new StyleSpec().italic())]) },
  { name: "embedded_nul_spans_4", make: () => View.styledText([TextSpan.plain("a\0"), TextSpan.plain("b\0"), TextSpan.plain("c\0"), TextSpan.plain("d\0")]) },
  { name: "long_text", make: () => View.text("0123456789abcdef".repeat(64)) },
  { name: "styled_chain", make: () => View.text("styled").bold().foreground("cyan").noWrap() },
  { name: "spans_1", make: () => View.styledText([TextSpan.plain("one")]) },
  { name: "spans_2", make: () => View.styledText([TextSpan.plain("one"), TextSpan.styled("two", new StyleSpec().italic())]) },
  { name: "spans_3", make: () => View.styledText([TextSpan.plain("one"), TextSpan.plain("two"), TextSpan.plain("three")]) },
  { name: "spans_4", make: () => View.styledText([TextSpan.plain("one"), TextSpan.plain("two"), TextSpan.plain("three"), TextSpan.plain("four")]) },
];

function median(values: readonly number[]): number {
  const sorted = [...values].sort((left, right) => left - right);
  return sorted[Math.floor(sorted.length / 2)]!;
}

function measure(host: InstanceType<NonNullable<typeof Host>>, make: () => View, route: "native" | "v4"): number {
  const run = (): void => {
    const view = make();
    if (route === "native") {
      const reference = tryNativeTextCreateRender(host, view);
      if (reference === undefined) throw new Error(`native string route rejected for ${route}`);
      releaseNativeViewRef(nativeViewAbiSession(), reference);
      return;
    }
    host.render(nodeForBridge(view));
  };
  for (let index = 0; index < WARMUP; index += 1) run();
  const samples: number[] = [];
  for (let repeat = 0; repeat < REPEATS; repeat += 1) {
    const start = Bun.nanoseconds();
    for (let iteration = 0; iteration < ITERATIONS; iteration += 1) run();
    samples.push(Number(Bun.nanoseconds() - start) / ITERATIONS);
  }
  return median(samples);
}

const session = nativeViewAbiSession();
if (session === undefined || Host === undefined) throw new Error("native View ABI is unavailable");
const results: Record<string, { native_median_ns: number; v4_median_ns: number }> = {};
for (const entry of cases) {
  const host = new Host(200, 4, true);
  try {
    results[entry.name] = {
      native_median_ns: measure(host, entry.make, "native"),
      v4_median_ns: measure(host, entry.make, "v4"),
    };
  } finally {
    host.dispose();
  }
}

const nativeArtifact = await Bun.file(new URL("../../../iyon-tui/native/iyon-tui-native.node", import.meta.url)).arrayBuffer();
const nativeArtifactSha256 = createHash("sha256").update(new Uint8Array(nativeArtifact)).digest("hex");
const git = (command: string): string => {
  const result = Bun.spawnSync(["git", ...command.split(" ")], { stdout: "pipe", stderr: "ignore" });
  return new TextDecoder().decode(result.stdout).trim();
};
console.log(JSON.stringify({
  benchmark: "PERF-11.9-native-strings",
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
  warmup: WARMUP,
  iterations: ITERATIONS,
  repeats: REPEATS,
  cases: cases.map((entry) => entry.name),
  routes: {
    native: "pending compact text -> style atoms -> generated cstring or buffer -> one host install",
    v4: "pre-materialized direct bridge -> existing host decoder",
  },
  results,
  structural_encoding_ns: 0,
  command_words_written: 0,
  path_arrays_written: 0,
  node_id_arrays_written: 0,
}, null, 2));
