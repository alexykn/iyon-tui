/**
 * PERF-12 T13 (§49/§77–§81) smoke evidence: the production boundary shape.
 *
 * Workload mirrors the traced production trace (PERF-12-production-boundary-trace.md):
 * a chrome scene whose footer text changes each operation (B1 retained install),
 * an exact-root render of an unchanged body object, a tool-card slot update
 * (B3) and a tool-output ScrollPane update + followEnd (B4), and one History
 * unit import (B2). Structural counters prove no boundary silently routes
 * through Direct/fallback on the retained trace.
 */

import { writeFileSync } from "node:fs";
import { native } from "../src/native.ts";
import { nativeViewAbiSession } from "../src/tui/native_view_abi.ts";
import {
  resetRetainedIdentityCounters,
  retainedIdentityCounterSnapshot,
} from "../src/tui/retained_dag.ts";
import { View } from "../src/tui/values/view.ts";
import { Style } from "../src/tui/values/style.ts";
import { Tui } from "../src/tui/runtime.ts";

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

async function runTrace(): Promise<{
  samples: number[];
  counters: ReturnType<typeof retainedIdentityCounterSnapshot>;
}> {
  if (nativeViewAbiSession() === undefined) throw new Error("T13 benchmark requires the staged native artifact");
  const runtime = await Tui.open({ width: 80, height: 24, headless: true });
  try {
    const composerSlot = runtime.createViewSlot(View.spacer(1));
    const cardSlot = runtime.createViewSlot(View.text("tool preparing").fillWidth());
    const outputPane = runtime.createScrollPane(View.spacer(0));
    const history = runtime.createHistory();
    history.setLayout({ padding: 1, gap: 1 });

    let generation = 0;
    // A stable tool-card shell reused across operations, in the EXACT
    // production shape from @iyon/plugins support helpers: hanging bullet
    // line (toolCallLine), hanging result line (toolResultLine), and the
    // collapseResultView clampRows(16, themed footer). Re-setting this View
    // must ride BridgeNativeHint cutoffs through the §76 hanging/clamp
    // materializers — zero payload reads, zero constructors.
    const callStyle = Style.new().foreground("theme:tool.finished");
    const resultStyle = Style.new().foreground("theme:text.muted");
    const truncationStyle = Style.new().foreground("theme:truncation_footer").italic().dim();
    const stableCardShell = View.vertical([
      View.hanging(
        View.text("● ").style(callStyle).noWrap(),
        View.text("  ").noWrap(),
        View.text("bash — finished").style(callStyle).fillWidth(),
      ).fillWidth(),
      View.hanging(
        View.text("  ").noWrap(),
        View.text("  ").noWrap(),
        View.text("exit 0").style(resultStyle).fillWidth(),
      ).fillWidth(),
    ]).clampRows(16, { kind: "footer", prefix: "… more lines (full result retained)", style: truncationStyle });
    const buildBody = (): View =>
      View.vertical((column) => {
        column.child(View.component(cardSlot).fillWidth());
        column.contentMax(13, View.component(composerSlot).fillWidth());
        column.child(View.text(`provider · model · effort: medium · ${generation}`).fillWidth());
      }).fillWidth();

    const operation = (): void => {
      // B1: changed frontier — new footer text only; composer/card identities stable.
      runtime.render({ body: buildBody(), history } as never);
      // Exact root: identical body object re-rendered (§20 shape inside Tui).
      const current = (runtime as unknown as { current(): { body: View } }).current()!.body;
      runtime.render({ body: current, history } as never);
      // B3/B4: live tool card pulse-free content swap + streamed pane append.
      generation += 1;
      cardSlot.setView(View.vertical([View.text(`tool step ${generation}`), View.text("args {}")]).fillWidth());
      outputPane.setContent(View.vertical([View.text(`out line ${generation}`)]).fillWidth());
      outputPane.followEnd();
      // §21 hint cutoff: the SAME shell View re-set rides its NativeRef hint.
      cardSlot.setView(stableCardShell);
      // B2: occasional finalized unit import riding shared identity.
      if (generation % 25 === 0) history.push(View.text(`finalized message ${generation}`).fillWidth());
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
    composerSlot.dispose();
    cardSlot.dispose();
    outputPane.dispose();
    history.dispose();
    return { samples, counters };
  } finally {
    await runtime.close();
  }
}

const result = await runTrace();
if (result.counters.cold_fallbacks !== 0) throw new Error(`T13 trace fell back ${result.counters.cold_fallbacks} times`);
const artifact = [
  JSON.stringify({
    record_kind: "t13_boundaries_smoke",
    profile: "smoke",
    benchmark_version: "PERF-12",
    candidate: "retained_dag_ffi",
    workload: "production_boundary_trace",
    mode: "RETAINED_TRACE",
    git_sha: commandText(["git", "rev-parse", "HEAD"]),
    perf7v2_sha: "e5292d62c4011610850cbdc1ba4a35f296f78e4f",
    perf11v4_result_sha: "7c670ccd99fb296b18719f62c1aa845a3e3605de",
    bun_version: Bun.version,
    bun_revision: commandText(["bun", "--revision"]),
    rustc_version: commandText(["rustc", "--version"]),
    target: commandText(["rustc", "-vV"]).split("host: ")[1]?.split("\n")[0] ?? "unknown",
    warmup_ops: WARMUP,
    measured_ops: MEASURED,
    semantic_construction_samples_ns: [],
    transport_prepare_samples_ns: [],
    native_materialize_samples_ns: [],
    host_commit_samples_ns: [],
    samples_ns: result.samples.map(Math.round),
    median_ns: Math.round(median(result.samples)),
    p95_ns: Math.round(percentile(result.samples, 0.95)),
    p99_ns: Math.round(percentile(result.samples, 0.99)),
    median_ci95_ns: bootstrapMedianCi95(result.samples).map(Math.round),
    structural: result.counters,
  }),
  JSON.stringify({
    record_kind: "t13_boundaries_provenance",
    profile: "smoke",
    benchmark_version: "PERF-12",
    git_sha: commandText(["git", "rev-parse", "HEAD"]),
    bun_version: Bun.version,
    bun_revision: commandText(["bun", "--revision"]),
    rustc_version: commandText(["rustc", "--version"]),
    target: commandText(["rustc", "-vV"]).split("host: ")[1]?.split("\n")[0] ?? "unknown",
    addon_sha256: commandText(["shasum", "-a", "256", "packages/iyon-runtime/native/iyon-native.node"]).split(" ")[0],
  }),
].join("\n") + "\n";
writeFileSync("packages/iyon-runtime/bench/PERF-12-t13-boundaries.jsonl", artifact);
console.log(artifact.split("\n")[0]);
