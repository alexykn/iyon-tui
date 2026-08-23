/**
 * PERF-12 T13.1 R0 gate — cold uncomposed construction cost through the
 * compose-helper fall-through layer vs ordinary direct construction
 * (handoff §32.1 R0: "measured cold uncomposed construction <=3% vs
 * pre-change baseline"; AMENDMENT-C §17.3 keeps the same gate for the final
 * scoped form).
 *
 * In R0 every helper is a pure §19 fall-through, so this benchmark measures
 * exactly what the tranche adds to ordinary View construction: one extra
 * non-inlined call frame per semantic operation, on top of the identical
 * public-API work both arms perform.
 *
 * Methodology: process-isolated script, 30 rounds x 2,000 ops per arm after
 * warmup; per-op nanoseconds from process.hrtime.bigint(); median/p95/min
 * across rounds. Arms interleave A/B/A/B so drift affects both equally.
 * Writes one JSONL record with provenance.
 */

import { writeFileSync } from "node:fs";
import { Style } from "../src/tui/values/style.ts";
import { Insets } from "../src/tui/values/geometry.ts";
import { View, type View as ViewValue } from "../src/tui/values/view.ts";
import {
  composeBackground,
  composeClampRows,
  composeComponent,
  composeContentMax,
  composeFillWidth,
  composePadding,
  composeSpacer,
  composeStyleSpec,
  composeStyleState,
  composeStyledText,
  composeText,
  composeVertical,
} from "../src/tui/internal-composition.ts";
import { TextSpan } from "../src/tui/values/text.ts";

const ROUNDS = 30;
const OPS_PER_ROUND = 2_000;
const WARMUP_OPS = 5_000;

// Production-like chrome skeleton (probe harness shape): spacer + styled
// working hint + composer shell + clamped footer text, all decorated.
/** Branded handle id; the value is irrelevant to construction cost. */
const HANDLE = 77 as never as import("../src/tui/types.ts").NativeHandleId;

function buildDirect(footer: string, effort: string): ViewValue {
  return View.vertical((column) => {
    column.child(View.spacer(0));
    column.child(
      View.styledText([TextSpan.plain("hint "), TextSpan.styled("active", Style.new().bold())]),
    );
    column.contentMax(
      13,
      View.component({ id: HANDLE })
        .styleState("iyon.agent.effort", effort)
        .style(Style.new().foreground("theme:text.muted"))
        .fillWidth(),
    );
    column.child(View.text(footer).padding(Insets.horizontal(1)).background("#101010").fillWidth());
  });
}

function buildComposed(footer: string, effort: string): ViewValue {
  return composeVertical((column) => {
    column.child(composeSpacer(0));
    column.child(
      composeStyledText([TextSpan.plain("hint "), TextSpan.styled("active", Style.new().bold())]),
    );
    column.contentMax(
      13,
      composeFillWidth(
        composeStyleSpec(
          composeStyleState(composeComponent({ id: HANDLE }), "iyon.agent.effort", effort),
          Style.new().foreground("theme:text.muted"),
        ),
      ),
    );
    column.child(
      composeFillWidth(composeBackground(composePadding(composeText(footer), Insets.horizontal(1)), "#101010")),
    );
  });
}

// Sanity: both arms must produce equivalent semantics before timing.
{
  const { nodeForBridge } = await import("../src/tui/values/view.ts");
  const strip = (_key: string, value: unknown): unknown => (_key === "id" ? undefined : value);
  const a = JSON.stringify(nodeForBridge(buildDirect("ready", "high")), strip);
  const b = JSON.stringify(nodeForBridge(buildComposed("ready", "high")), strip);
  if (a !== b) {
    throw new Error(`R0 fall-through arms diverge semantically:\ndirect:   ${a}\ncomposed: ${b}`);
  }
}

function measure(build: (footer: string, effort: string) => ViewValue): number[] {
  let sink: ViewValue | undefined;
  const samples: number[] = [];
  for (let round = 0; round < ROUNDS; round += 1) {
    const start = process.hrtime.bigint();
    for (let op = 0; op < OPS_PER_ROUND; op += 1) {
      sink = build(op % 2 === 0 ? "ready" : "running tool 3/7", ["low", "medium", "high"][op % 3]);
    }
    const elapsedNs = Number(process.hrtime.bigint() - start);
    samples.push(Math.round(elapsedNs / OPS_PER_ROUND));
  }
  if (sink === undefined) throw new Error("unreachable");
  return samples;
}

function statistics(samples: number[]): { median: number; p95: number; min: number } {
  const sorted = samples.slice().sort((x, y) => x - y);
  const at = (q: number): number => sorted[Math.min(sorted.length - 1, Math.floor(q * sorted.length))];
  return { median: at(0.5), p95: at(0.95), min: sorted[0] };
}

// Warmup both arms.
for (let index = 0; index < WARMUP_OPS; index += 1) {
  buildDirect("warmup", "low");
  buildComposed("warmup", "low");
}

// Interleaved measurement.
const directSamples: number[] = [];
const composedSamples: number[] = [];
for (let round = 0; round < ROUNDS; round += 1) {
  const order = round % 2 === 0;
  const first = measure(order ? buildDirect : buildComposed);
  const second = measure(order ? buildComposed : buildDirect);
  directSamples.push(...(order ? first : second));
  composedSamples.push(...(order ? second : first));
}

const direct = statistics(directSamples);
const composed = statistics(composedSamples);
const deltaPct = ((composed.median - direct.median) / direct.median) * 100;

const record = {
  record_kind: "t13_1_r0_cold_fallthrough",
  profile: "smoke",
  benchmark_version: "PERF-12",
  tranche: "T13.1",
  step: "R0",
  git_sha: new TextDecoder().decode(Bun.spawnSync(["git", "rev-parse", "HEAD"]).stdout).trim(),
  bun_version: Bun.version,
  bun_revision: Bun.revision,
  rounds: ROUNDS,
  ops_per_round: OPS_PER_ROUND,
  warmup_ops: WARMUP_OPS,
  direct_median_ns: direct.median,
  direct_p95_ns: direct.p95,
  direct_min_ns: direct.min,
  composed_median_ns: composed.median,
  composed_p95_ns: composed.p95,
  composed_min_ns: composed.min,
  composed_overhead_pct: Number(deltaPct.toFixed(3)),
  gate: "cold uncomposed construction through helpers <= 3% over direct construction",
  gate_pass: deltaPct <= 3,
};

writeFileSync("packages/iyon-runtime/bench/PERF-12-T13.1-R0-cold-fallthrough.jsonl", `${JSON.stringify(record)}\n`);
console.log(JSON.stringify(record, null, 2));
if (!record.gate_pass) {
  console.error(`R0 GATE FAILED: composed median ${composed.median} ns vs direct ${direct.median} ns (${deltaPct.toFixed(2)}%)`);
  process.exitCode = 1;
}
