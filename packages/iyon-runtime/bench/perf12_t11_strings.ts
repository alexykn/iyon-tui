/** PERF-12 T11 §98/§103 smoke evidence: string path choice and payload lanes. */

import { writeFileSync } from "node:fs";
import { native } from "../src/native.ts";
import { nativeViewAbiSession } from "../src/tui/native_view_abi.ts";
import { nodeForBridge, View } from "../src/tui/values/view.ts";
import { TextSpan } from "../src/tui/values/text.ts";
import { StyleSpec } from "../src/tui/values/style.ts";
import {
  retainedIdentityCounterSnapshot,
  resetRetainedIdentityCounters,
  RetainedRootBoundary,
} from "../src/tui/retained_dag.ts";
import { viewTextCreateCstring, viewTextCreateUtf8 } from "../src/tui/generated/view_calls.ts";

type Host = { render(view: object): void; tuiViewAbiHostPointer(): number; dispose(): void };
const Host = native.NativeTuiHost as unknown as
  | (new (width: number, height: number, headless: boolean) => Host)
  | undefined;
const WARMUP = 50;
const MEASURED = 500;

function median(values: number[]): number {
  const sorted = [...values].sort((a, b) => a - b);
  const middle = sorted.length >> 1;
  return sorted.length % 2 === 0 ? (sorted[middle - 1]! + sorted[middle]!) / 2 : sorted[middle]!;
}

function percentile(values: number[], fraction: number): number {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * fraction))]!;
}

function bootstrapMedianCi95(values: number[], resamples = 1_000): [number, number] {
  const medians: number[] = [];
  for (let sample = 0; sample < resamples; sample += 1) {
    const draw = Array.from({ length: values.length }, () => values[(Math.random() * values.length) | 0]!);
    medians.push(median(draw));
  }
  medians.sort((left, right) => left - right);
  return [medians[Math.floor(resamples * 0.025)]!, medians[Math.floor(resamples * 0.975)]!];
}

function commandText(command: string[]): string {
  return new TextDecoder().decode(Bun.spawnSync(command).stdout).trim() || "unknown";
}

interface CaseResult {
  readonly dataset: string;
  readonly lane: string;
  readonly samples: number[];
  readonly counters: ReturnType<typeof retainedIdentityCounterSnapshot>;
}

function runCase(
  session: ReturnType<typeof nativeViewAbiSession>,
  boundary: RetainedRootBoundary,
  datasetName: string,
  build: () => View,
  laneOverride?: string,
): CaseResult {
  if (session === undefined) throw new Error("T11 benchmark requires the staged native artifact");
  const operation = (): void => {
    // Each operation constructs a fresh semantic frontier; the previous root
    // stays leased until the replacement is installed (§18).
    if (boundary.install(build()) === undefined) throw new Error("T11 benchmark install fell back");
  };
  for (let index = 0; index < WARMUP; index += 1) operation();
  resetRetainedIdentityCounters();
  const samples: number[] = [];
  for (let index = 0; index < MEASURED; index += 1) {
    const started = Bun.nanoseconds();
    operation();
    samples.push(Number(Bun.nanoseconds() - started));
  }
  const counters = retainedIdentityCounterSnapshot();
  // Lane attribution from structural counters: the exact-byte tier moves JS
  //-encoded bytes; the cstring family moves none.
  const lane = laneOverride ?? (counters.byte_payload_bytes > 0 ? "utf8-exact-byte" : "cstring");
  return { dataset: "", lane, samples, counters };
}

const session = nativeViewAbiSession();
if (Host === undefined || session === undefined) throw new Error("T11 benchmark requires the staged native artifact");
const host = new Host(80, 24, true);
const boundary = new RetainedRootBoundary(session, () => host.tuiViewAbiHostPointer() as never);

const styledSpans = [
  TextSpan.plain("plain "),
  TextSpan.styled("red", new StyleSpec().foreground("#ff0000")),
  TextSpan.styled(" bold", new StyleSpec().bold()),
  TextSpan.styled(" themed", new StyleSpec().theme("t11.bench")),
];
const diffLines = Array.from({ length: 40 }, (_, index) => {
  const kind = index % 3 === 0 ? "deletion" : index % 3 === 1 ? "addition" : "context";
  const line: Record<string, unknown> = {
    kind,
    text: `${kind} line ${index} ✓`,
    termination: index % 7 === 0 ? "unterminated" : "terminated",
  };
  if (kind !== "addition") (line as { oldLine?: number }).oldLine = index + 1;
  if (kind !== "deletion") (line as { newLine?: number }).newLine = index + 1;
  return line;
});

// §98 datasets. Every case wraps its payload so each operation materializes
// exactly one new text/diff node plus one new column.
const datasets: readonly [string, () => View][] = [
  ["short_ascii", () => View.vertical([View.text("the quick brown fox")])],
  ["short_unicode", () => View.vertical([View.text("héllo ✓ 世界")])],
  ["emoji_non_bmp", () => View.vertical([View.text("family 👨‍👩‍👧 flag 🇺🇸")])],
  ["embedded_nul", () => View.vertical([View.text("a\u0000b ✓")])],
  ["bytes_256", () => View.vertical([View.text("x".repeat(256))])],
  ["bytes_4k", () => View.vertical([View.text("y".repeat(4096))])],
  ["styled_spans", () => View.vertical([View.styledText(styledSpans.map((span) => span))])],
  [
    "diff_lines",
    () => View.vertical([View.diff([{
      oldRange: { start: 0, count: 27 },
      newRange: { start: 0, count: 26 },
      lines: diffLines as never,
    }])]),
  ],
];

const caseRecords: string[] = [];
for (const [name, build] of datasets) {
  const result = runCase(session, boundary, name, build, name === "diff_lines" ? "diff-words-bytes" : undefined);
  const samples = result.samples.map(Math.round);
  caseRecords.push(JSON.stringify({
    record_kind: "t11_string_case",
    profile: "smoke",
    benchmark_version: "PERF-12",
    candidate: "retained_dag_ffi",
    workload: name,
    lane: result.lane,
    git_sha: commandText(["git", "rev-parse", "HEAD"]),
    bun_version: Bun.version,
    bun_revision: commandText(["bun", "--revision"]),
    rustc_version: commandText(["rustc", "--version"]),
    target: commandText(["rustc", "-vV"]).split("host: ")[1]?.split("\n")[0] ?? "unknown",
    warmup_ops: WARMUP,
    measured_ops: MEASURED,
    samples_ns: samples,
    median_ns: Math.round(median(result.samples)),
    p95_ns: Math.round(percentile(result.samples, 0.95)),
    p99_ns: Math.round(percentile(result.samples, 0.99)),
    median_ci95_ns: bootstrapMedianCi95(result.samples).map(Math.round),
    structural: result.counters,
  }));
}

// Lane comparison on identical payloads through the raw constructor shapes
// (both lanes are legal whenever no span carries an embedded NUL). This is
// the end-to-end measurement backing the default-lane rule: prefer cstring
// unless the byte lane wins or NUL forces it.
const laneProbePayloads: readonly [string, string][] = [
  ["ascii", "the quick brown fox jumps over the lazy dog"],
  ["unicode", "héllo ✓ 世界 combined e\u0301"],
];
const laneProbes: string[] = [];
{
  // Each probe iteration mints a fresh NodeId so every call is the real
  // cold-construction shape; a fixed live NodeId would hit the §23
  // semantic-cache consult after the first call and measure nothing.
  let probeNodeId = 900_000;
  const nextId = (): [number, number] => {
    probeNodeId += 1;
    return [probeNodeId % 0x1_0000_0000, Math.floor(probeNodeId / 0x1_0000_0000)];
  };
  const encoder = new TextEncoder();
  const scratch = new Uint8Array(4096);
  for (const [name, text] of laneProbePayloads) {
    const timed = (lower: () => number): number[] => {
      for (let index = 0; index < 10_000; index += 1) lower();
      const samples: number[] = [];
      for (let index = 0; index < 10_000; index += 1) {
        const started = Bun.nanoseconds();
        lower();
        samples.push(Number(Bun.nanoseconds() - started));
      }
      return samples;
    };
    const cstringSamples = timed(() => {
      const [idLow, idHigh] = nextId();
      return viewTextCreateCstring(session.symbols, session.runtime, idLow, idHigh, text, 0, 1, 1);
    });
    const encoded = encoder.encode(text);
    scratch.set(encoded);
    const utf8Samples = timed(() => {
      const [idLow, idHigh] = nextId();
      return viewTextCreateUtf8(session.symbols, session.runtime, idLow, idHigh, scratch, encoded.length, 0, 1, 1);
    });
    laneProbes.push(JSON.stringify({
      record_kind: "t11_lane_probe",
      profile: "smoke",
      benchmark_version: "PERF-12",
      workload: name,
      payload_bytes: encoded.length,
      cstring_median_ns: Math.round(median(cstringSamples)),
      utf8_median_ns: Math.round(median(utf8Samples)),
      warmup_ops: 10_000,
      measured_ops: 10_000,
    }));
  }
}

boundary.close();
host.dispose();

const artifact = [
  JSON.stringify({
    record_kind: "t11_strings_provenance",
    profile: "smoke",
    benchmark_version: "PERF-12",
    git_sha: commandText(["git", "rev-parse", "HEAD"]),
    bun_version: Bun.version,
    bun_revision: commandText(["bun", "--revision"]),
    rustc_version: commandText(["rustc", "--version"]),
    target: commandText(["rustc", "-vV"]).split("host: ")[1]?.split("\n")[0] ?? "unknown",
    addon_sha256: commandText(["shasum", "-a", "256", "packages/iyon-runtime/native/iyon-native.node"]).split(" ")[0],
  }),
  ...caseRecords,
  ...laneProbes,
].join("\n") + "\n";
writeFileSync("packages/iyon-runtime/bench/PERF-12-t11-strings.jsonl", artifact);
console.log(artifact);
