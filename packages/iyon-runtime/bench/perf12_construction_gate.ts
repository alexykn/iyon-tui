/**
 * PERF-12 T4 pre-implementation challenge benchmark (PERF-12 handoff §85).
 *
 * Question (user-directed): is the faithful 7v2 eager semantic View DAG
 * actually better than the current production pending-backing View, or does
 * the pending model win?
 *
 * Arms:
 *   prod_construct    current values/view.ts View construction only
 *   prod_ready        production construction + nodeForBridge() (render-ready
 *                     state: this is what every rendered View pays at least
 *                     once, since render/history/slots call nodeForBridge)
 *   v7_construct      faithful 7v2 eager frozen-node construction (perf12_view_7v2.ts)
 *   v7_ready          7v2 construction + nodeForBridge() (WeakMap lookup)
 *
 * Cases follow §85 plus a realistic agent-message-shaped mixed tree.
 * Timing discipline per §102 tiny cases: Bun.gc(true) before each block,
 * 1,000 ops per timed block, order rotated per round, >=30 warmup rounds,
 * 50 measured rounds; per-op medians reported from block medians.
 */

import { View as ProdView, nodeForBridge, textRowsForHarness } from "../src/tui/values/view.ts";
import { TextSpan } from "../src/tui/values/text.ts";
import { StyleSpec } from "../src/tui/values/style.ts";
import { View as V7View, nodeForBridge as nodeForBridgeV7, textRowsForHarness as textRowsV7 } from "./perf12_view_7v2.ts";

interface Case {
  readonly name: string;
  buildProduction(): ProdView;
  buildV7(): V7View;
}

function styleOf(foreground: string): StyleSpec {
  return new StyleSpec({ foreground, attributes: {} });
}

const CASES: Case[] = [
  {
    name: "plain_text",
    buildProduction: () => ProdView.text("hello world"),
    buildV7: () => V7View.text("hello world"),
  },
  {
    name: "styled_text_3spans",
    buildProduction: () => ProdView.styledText([
      new TextSpan({ text: "error", style: styleOf("red").value }),
      new TextSpan({ text: ": file not found in " }),
      new TextSpan({ text: "src/main.ts", style: styleOf("cyan").value }),
    ]),
    buildV7: () => V7View.styledText([
      new TextSpan({ text: "error", style: styleOf("red").value }),
      new TextSpan({ text: ": file not found in " }),
      new TextSpan({ text: "src/main.ts", style: styleOf("cyan").value }),
    ]),
  },
  {
    name: "modifier_chain_3",
    buildProduction: () => ProdView.text("status").bold().style(styleOf("green")).padding(1),
    buildV7: () => V7View.text("status").bold().style(styleOf("green")).padding(1),
  },
  {
    name: "column_20",
    buildProduction: () => ProdView.vertical(
      Array.from({ length: 20 }, (_, i) => ProdView.text(`line ${i}`)),
    ),
    buildV7: () => V7View.vertical(
      Array.from({ length: 20 }, (_, i) => V7View.text(`line ${i}`)),
    ),
  },
  {
    name: "column_200",
    buildProduction: () => ProdView.vertical(
      Array.from({ length: 200 }, (_, i) =>
        i % 5 === 0 ? ProdView.text(`header ${i}`).bold() : ProdView.text(`row ${i}`)),
    ),
    buildV7: () => V7View.vertical(
      Array.from({ length: 200 }, (_, i) =>
        i % 5 === 0 ? V7View.text(`header ${i}`).bold() : V7View.text(`row ${i}`)),
    ),
  },
  {
    name: "row_tracks_mixed",
    buildProduction: () => ProdView.horizontal((row) => {
      row.gap(1);
      row.fixed(8, ProdView.text("fixed"));
      row.flex(ProdView.text("flex"));
      row.child(ProdView.text("normal"));
    }),
    buildV7: () => V7View.horizontal((row) => {
      row.gap(1);
      row.fixed(8, V7View.text("fixed"));
      row.flex(V7View.text("flex"));
      row.child(V7View.text("normal"));
    }),
  },
  {
    name: "grid_3x3",
    buildProduction: () => ProdView.grid((grid) => {
      grid.columns([{ kind: "fixed", size: 12 }, { kind: "content" }, { kind: "flex" }]);
      for (let r = 0; r < 3; r += 1) {
        grid.row((row) => {
          for (let c = 0; c < 3; c += 1) row.cell(ProdView.text(`cell-${r}-${c}`));
        });
      }
    }),
    buildV7: () => V7View.grid((grid) => {
      grid.columns([{ kind: "fixed", size: 12 }, { kind: "content" }, { kind: "flex" }]);
      for (let r = 0; r < 3; r += 1) {
        grid.row((row) => {
          for (let c = 0; c < 3; c += 1) row.cell(V7View.text(`cell-${r}-${c}`));
        });
      }
    }),
  },
  {
    name: "diff_10_lines",
    buildProduction: () => ProdView.diff([{
      oldRange: { start: 2, count: 4 },
      newRange: { start: 2, count: 6 },
      lines: [
        { kind: "context", text: "unchanged line", termination: "terminated" },
        { kind: "deletion", text: "old line", termination: "terminated" },
        { kind: "addition", text: "new line", termination: "terminated" },
        { kind: "context", text: "more context", termination: "unterminated" },
        { kind: "addition", text: "another addition", termination: "terminated" },
      ],
    }]),
    buildV7: () => V7View.diff([{
      oldRange: { start: 2, count: 4 },
      newRange: { start: 2, count: 6 },
      lines: [
        { kind: "context", text: "unchanged line", termination: "terminated" },
        { kind: "deletion", text: "old line", termination: "terminated" },
        { kind: "addition", text: "new line", termination: "terminated" },
        { kind: "context", text: "more context", termination: "unterminated" },
        { kind: "addition", text: "another addition", termination: "terminated" },
      ],
    }]),
  },
  {
    name: "agent_message_realistic",
    buildProduction: () => ProdView.vertical([
      ProdView.text("assistant").bold().style(styleOf("magenta")),
      ProdView.text("Here is the analysis you asked for, with details below.").wrap("wordThenGrapheme"),
      ProdView.vertical(Array.from({ length: 6 }, (_, i) =>
        ProdView.horizontal((row) => {
          row.fixed(6, ProdView.text(i % 2 === 0 ? "done" : "wait").style(styleOf(i % 2 === 0 ? "green" : "yellow")));
          row.child(ProdView.text(`step ${i}: some longer description of the step`));
        }))),
      ProdView.text("$ bun test").style(new StyleSpec({ background: "black", attributes: {} })).padding(1),
    ]),
    buildV7: () => V7View.vertical([
      V7View.text("assistant").bold().style(styleOf("magenta")),
      V7View.text("Here is the analysis you asked for, with details below.").wrap("wordThenGrapheme"),
      V7View.vertical(Array.from({ length: 6 }, (_, i) =>
        V7View.horizontal((row) => {
          row.fixed(6, V7View.text(i % 2 === 0 ? "done" : "wait").style(styleOf(i % 2 === 0 ? "green" : "yellow")));
          row.child(V7View.text(`step ${i}: some longer description of the step`));
        }))),
      V7View.text("$ bun test").style(new StyleSpec({ background: "black", attributes: {} })).padding(1),
    ]),
  },
];

type Arm = "prod_construct" | "prod_ready" | "v7_construct" | "v7_ready";
const ARMS: Arm[] = ["prod_construct", "prod_ready", "v7_construct", "v7_ready"];

function runBlock(caseSpec: Case, arm: Arm, ops: number): number {
  const start = Bun.nanoseconds();
  for (let i = 0; i < ops; i += 1) {
    switch (arm) {
      case "prod_construct": caseSpec.buildProduction(); break;
      case "prod_ready": nodeForBridge(caseSpec.buildProduction()); break;
      case "v7_construct": caseSpec.buildV7(); break;
      case "v7_ready": nodeForBridgeV7(caseSpec.buildV7()); break;
    }
  }
  return Number(Bun.nanoseconds() - start) / ops;
}

function median(samples: number[]): number {
  const sorted = [...samples].sort((a, b) => a - b);
  const mid = sorted.length >> 1;
  return sorted.length % 2 === 0 ? (sorted[mid - 1]! + sorted[mid]!) / 2 : sorted[mid]!;
}

function p95(samples: number[]): number {
  const sorted = [...samples].sort((a, b) => a - b);
  return sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * 0.95))]!;
}

async function main() {
  const OPS = 1_000;
  const WARMUP_ROUNDS = 30;
  const MEASURED_ROUNDS = 50;

  // Sanity: both models must be render-ready and produce equivalent harness
  // rows. Structural JSON equality is NOT expected: production pushes
  // modifier styles into text spans while faithful 7v2 keeps them on the
  // decoration wrapper (render-equivalent, structurally different).
  for (const caseSpec of CASES) {
    const prodView = caseSpec.buildProduction();
    const v7View = caseSpec.buildV7();
    if (nodeForBridge(prodView).kind !== nodeForBridgeV7(v7View).kind) {
      throw new Error(`kind mismatch in case ${caseSpec.name}`);
    }
    const prodRows = JSON.stringify(textRowsForHarness(prodView));
    const v7Rows = JSON.stringify(textRowsV7(v7View));
    if (prodRows !== v7Rows) {
      throw new Error(`rendered-row mismatch in case ${caseSpec.name}: ${prodRows} vs ${v7Rows}`);
    }
  }

  const samples: Record<Arm, number[]> = { prod_construct: [], prod_ready: [], v7_construct: [], v7_ready: [] };
  for (let round = 0; round < WARMUP_ROUNDS + MEASURED_ROUNDS; round += 1) {
    Bun.gc(true);
    const order = ARMS.slice(round % ARMS.length).concat(ARMS.slice(0, round % ARMS.length));
    for (const caseSpec of CASES) {
      for (const arm of order) {
        const nsPerOp = runBlock(caseSpec, arm, OPS);
        if (round >= WARMUP_ROUNDS) samples[arm].push(nsPerOp);
      }
    }
  }

  // Per-case samples were appended per round; re-run measurement keeping
  // per-case separation for reporting.
  const perCase: Record<string, Record<Arm, number[]>> = {};
  for (const caseSpec of CASES) perCase[caseSpec.name] = { prod_construct: [], prod_ready: [], v7_construct: [], v7_ready: [] };
  for (let round = 0; round < WARMUP_ROUNDS + MEASURED_ROUNDS; round += 1) {
    Bun.gc(true);
    const order = ARMS.slice(round % ARMS.length).concat(ARMS.slice(0, round % ARMS.length));
    for (const caseSpec of CASES) {
      for (const arm of order) {
        const nsPerOp = runBlock(caseSpec, arm, OPS);
        if (round >= WARMUP_ROUNDS) perCase[caseSpec.name]![arm]!.push(nsPerOp);
      }
    }
  }
  void samples;

  const records: Record<string, unknown>[] = [];
  console.log("case                       prod_construct  prod_ready   v7_construct  v7_ready     ready_ratio(v7/prod)");
  for (const caseSpec of CASES) {
    const stats = Object.fromEntries(ARMS.map((arm) => {
      const values = perCase[caseSpec.name]![arm]!;
      return [arm, { median: median(values), p95: p95(values) }];
    })) as Record<Arm, { median: number; p95: number }>;
    const ratio = stats.v7_ready.median / stats.prod_ready.median;
    console.log(
      caseSpec.name.padEnd(24),
      String(Math.round(stats.prod_construct.median)).padStart(10) + "ns",
      String(Math.round(stats.prod_ready.median)).padStart(9) + "ns",
      String(Math.round(stats.v7_construct.median)).padStart(10) + "ns",
      String(Math.round(stats.v7_ready.median)).padStart(9) + "ns",
      ratio.toFixed(3).padStart(12),
    );
    records.push({
      record_kind: "construction_challenge_case",
      profile: "gate",
      case: caseSpec.name,
      ops_per_block: OPS,
      ...stats,
      v7_ready_over_prod_ready_median_ratio: ratio,
    });
  }

  const commandText = (command: string[]): string =>
    new TextDecoder().decode(Bun.spawnSync(command).stdout).trim() || "unknown";
  const summary = {
    record_kind: "construction_challenge_summary",
    profile: "gate",
    git_sha: commandText(["git", "rev-parse", "HEAD"]),
    bun_version: Bun.version,
    bun_revision: commandText(["bun", "--revision"]),
    rustc_version: commandText(["rustc", "--version"]),
    macos_version: commandText(["sw_vers", "-productVersion"]),
    cpu_model: commandText(["sysctl", "-n", "machdep.cpu.brand_string"]),
    warmup_rounds: WARMUP_ROUNDS,
    measured_rounds: MEASURED_ROUNDS,
  };
  records.push(summary);
  const outPath = Bun.env.PERF12_CONSTRUCTION_OUT ?? "packages/iyon-runtime/bench/PERF-12-construction-challenge.jsonl";
  await Bun.write(outPath, records.map((record) => JSON.stringify(record)).join("\n") + "\n");
  console.log(`\nwrote ${outPath}`);
}

await main();
