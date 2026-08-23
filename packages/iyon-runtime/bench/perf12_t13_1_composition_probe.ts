/**
 * PERF-12 T13.1 Step 1 (§36/§37/§48): production-chrome evidence baseline
 * through the REAL production Tui.render router, captured BEFORE any
 * composition runtime exists.
 *
 * Arms:
 *   current_body_key     - the app's string-memo guard + ordinary construction
 *   rebuild_uncomposed   - no guard; fresh declarative construction every op
 *   manual_stable_oracle - hand-preserved View identities per logical chrome
 *                          part; the experimental upper bound that automatic
 *                          composition must reproduce structurally (§45.1)
 *
 * (The fourth §36 arm, composed_auto, activates with the T13.1 composition
 * runtime and reuses this exact harness.)
 *
 * Cases model plugins/app/iyon state transitions (§37): exact semantic no-op,
 * footer-only change, effort style-state change, working-row visibility
 * toggle, approval toggle, steering preview change, tool status update behind
 * a retained ViewSlot (chrome unchanged), tool pane output update.
 *
 * Every arm drives the identical deterministic state sequence through its own
 * headless Tui session; final screens must match across arms (parity check).
 */

import { writeFileSync } from "node:fs";
import {
  resetRetainedIdentityCounters,
  retainedIdentityCounterSnapshot,
} from "../src/tui/retained_dag.ts";
import { Style } from "../src/tui/values/style.ts";
import { Insets } from "../src/tui/values/geometry.ts";
import { View, nodeForBridge, type View as ViewValue } from "../src/tui/values/view.ts";
import { Scene } from "../src/tui/scene.ts";
import { History } from "../src/tui/history.ts";
import type { ScrollPane } from "../src/tui/types.ts";
import { TextInput } from "../src/tui/text-input.ts";
import { ViewSlot } from "../src/tui/component.ts";
import { Tui } from "../src/tui/runtime.ts";

const WARMUP = 50;
const MEASURED = 500;
const WIDTH = 80;
const HEIGHT = 24;

type Effort = "minimal" | "low" | "medium" | "high";
const EFFORTS: readonly Effort[] = ["minimal", "low", "medium", "high"];
const STATUSES = ["waiting", "streaming", "running tool", "idle"] as const;
const TOOL_STATUSES = ["preparing", "running", "finishing"] as const;

interface ProbeState {
  provider: string;
  modelId: string;
  effort: Effort;
  status: string;
  activityVisible: boolean;
  steering: readonly string[];
  approvalId: string | undefined;
  /** Tool status rides the app bodyKey even though the chrome never shows it. */
  toolStatus: string;
  /** State that changes without touching any chrome input (§37.1). */
  hiddenCounter: number;
}

function initialState(): ProbeState {
  return {
    provider: "anthropic",
    modelId: "ox-alpha",
    effort: "medium",
    status: "waiting",
    activityVisible: true,
    steering: [],
    approvalId: undefined,
    toolStatus: "preparing",
    hiddenCounter: 0,
  };
}

const COMPOSER_STYLE = Style.new();
const FOOTER_STYLE = Style.new().dim();
const MUTED_COLOR = "theme:text.muted" as const;

function muted(text: string): ViewValue {
  return View.text(text).noWrap().italic().foreground(MUTED_COLOR);
}

function footerText(state: ProbeState): string {
  const effort = { minimal: "Minimal", low: "Low", medium: "Medium", high: "High" }[state.effort];
  return [state.provider, state.modelId, `effort: ${effort}`, state.status].filter((value) => value.length > 0).join(" · ");
}

function workingQueuePreview(state: ProbeState): string | undefined {
  const first = state.steering[0];
  if (first === undefined) return undefined;
  return first.split(/\s+/).filter(Boolean).join(" ");
}

/** The app's bodyKey over the probe's chrome-relevant state slice. */
function bodyKey(state: ProbeState): string {
  return [
    footerText(state),
    state.effort,
    state.approvalId ?? "",
    Number(state.activityVisible),
    state.steering.join("\u0001"),
    state.toolStatus,
  ].join("|");
}

interface ChromeHandles {
  composer: TextInput;
  workingSlot: ViewSlot;
}

// --- Ordinary declarative construction (one fresh View value per call). ----

function buildWorkingPart(state: ProbeState, handles: ChromeHandles): ViewValue {
  if (!state.activityVisible) return View.spacer(0);
  const preview = workingQueuePreview(state);
  const extra = state.steering.length - 1;
  return View.horizontal((row) => {
    row.gap(4);
    row.child(View.component(handles.workingSlot));
    if (preview !== undefined) {
      row.flex(muted(`Queue: ${preview}`));
      if (extra > 0) row.child(muted(` + ${extra} more`));
    }
  })
    .fillWidth()
    .padding(Insets.of(0, 2, 1, 2));
}

function buildApprovalPart(state: ProbeState): ViewValue {
  if (state.approvalId === undefined) return View.spacer(0);
  return View.text(`Approve bash? Press Enter to approve or Escape to reject (${state.approvalId}).`).fillWidth();
}

function buildComposerPart(state: ProbeState, handles: ChromeHandles): ViewValue {
  return View.component(handles.composer)
    .style(COMPOSER_STYLE)
    .styleState("iyon.agent.effort", state.effort)
    .fillWidth();
}

function buildFooterPart(state: ProbeState): ViewValue {
  return View.text(footerText(state)).style(FOOTER_STYLE).fillWidth();
}

function buildRoot(working: ViewValue, approval: ViewValue, composer: ViewValue, footer: ViewValue): ViewValue {
  return View.vertical((column) => {
    column.child(working);
    column.child(approval);
    column.contentMax(13, composer);
    column.child(footer);
  })
    .fillWidth()
    .fillHeight();
}

function buildChromeFresh(state: ProbeState, handles: ChromeHandles): ViewValue {
  return buildRoot(
    buildWorkingPart(state, handles),
    buildApprovalPart(state),
    buildComposerPart(state, handles),
    buildFooterPart(state),
  );
}

// --- Manual stable oracle: hand-preserved identity per logical part. -------
// This is the structural upper bound automatic composition must match:
// unchanged parts return the EXACT previous View object; only the changed
// path back to the root is newly constructed. Keys are SEMANTIC INPUTS here
// (the app knows its own state); the future composed_auto arm must reach the
// same reuse decisions WITHOUT application-provided keys, by comparing each
// site's immediate semantic inputs against the previous committed View.

class ManualChromeOracle {
  private working?: { key: string; view: ViewValue };
  private approval?: { key: string; view: ViewValue };
  private composer?: { key: string; view: ViewValue };
  private footer?: { key: string; view: ViewValue };
  private root?: { childrenKey: string; view: ViewValue };

  body(state: ProbeState, handles: ChromeHandles): ViewValue {
    const workingKey = `${state.activityVisible}|${workingQueuePreview(state) ?? ""}|${Math.max(state.steering.length - 1, 0)}`;
    if (this.working?.key !== workingKey) this.working = { key: workingKey, view: buildWorkingPart(state, handles) };
    const approvalKey = state.approvalId ?? "";
    if (this.approval?.key !== approvalKey) this.approval = { key: approvalKey, view: buildApprovalPart(state) };
    if (this.composer?.key !== state.effort) this.composer = { key: state.effort, view: buildComposerPart(state, handles) };
    const footerKey = footerText(state);
    if (this.footer?.key !== footerKey) this.footer = { key: footerKey, view: buildFooterPart(state) };

    // Exact no-op shape: when every child identity is unchanged, return the
    // exact previous root object so Tui.render takes its object-identity
    // no-op route. Child keys are BridgeViewNode identities (lookup-only),
    // NOT payload content: equal-content Views are different semantic nodes
    // and MUST produce different keys.
    const childrenKey = [
      this.working.view,
      this.approval.view,
      this.composer.view,
      this.footer.view,
    ].map((view) => nodeForBridge(view).id).join("|");
    if (this.root?.childrenKey !== childrenKey) {
      this.root = {
        childrenKey,
        view: buildRoot(this.working.view, this.approval.view, this.composer.view, this.footer.view),
      };
    }
    return this.root.view;
  }
}

// --- Case scripts (deterministic §37 state transitions). -------------------

interface CaseScript {
  name: string;
  /** Advance the rolling state to the next transition of this case. */
  advance(state: ProbeState, op: number): void;
  /** Extra boundary work performed inside every op of this case (B3/B4). */
  boundaryOp?(op: number, extras: { cardSlot: ViewSlot; pane: ScrollPane }): void;
}

const CASES: readonly CaseScript[] = [
  {
    name: "exact_noop",
    advance: (state) => {
      state.hiddenCounter += 1;
    },
  },
  {
    name: "footer_only",
    advance: (state, op) => {
      state.status = STATUSES[op % STATUSES.length];
    },
  },
  {
    name: "effort_style_state",
    advance: (state, op) => {
      state.effort = EFFORTS[op % EFFORTS.length];
    },
  },
  {
    name: "working_toggle",
    advance: (state) => {
      state.activityVisible = !state.activityVisible;
      state.steering = [];
    },
  },
  {
    name: "approval_toggle",
    advance: (state, op) => {
      state.approvalId = op % 2 === 0 ? "appr-1" : undefined;
    },
  },
  {
    name: "steering_preview",
    advance: (state, op) => {
      state.activityVisible = true;
      state.steering = [`review the ${["auth", "render", "cache"][op % 3]} module carefully`];
    },
  },
  {
    name: "tool_slot_update",
    // Tool status is part of the app bodyKey even though the scene chrome
    // never displays it: arms A/B re-render the whole chrome per op while the
    // only real change lives behind the retained card slot (§37.7).
    advance: (state, op) => {
      state.toolStatus = TOOL_STATUSES[op % TOOL_STATUSES.length];
    },
    boundaryOp: (op, { cardSlot }) => {
      cardSlot.setView(
        View.vertical([
          View.text(`\u25CF bash \u2014 ${TOOL_STATUSES[op % TOOL_STATUSES.length]}`).fillWidth(),
          View.text("  exit 0").fillWidth(),
        ]).fillWidth(),
      );
    },
  },
  {
    name: "pane_output_update",
    advance: () => {},
    boundaryOp: (op, { pane }) => {
      pane.setContent(View.vertical([View.text(`out line ${op}`)]).fillWidth());
      pane.followEnd();
    },
  },
];

// --- Arms. ------------------------------------------------------------------

type Counters = ReturnType<typeof retainedIdentityCounterSnapshot>;

function counterDelta(before: Counters, after: Counters): Counters {
  const delta = {} as Record<string, number>;
  for (const key of Object.keys(before) as (keyof Counters)[]) {
    delta[key] = after[key] - before[key];
  }
  return delta as unknown as Counters;
}

interface ArmContext {
  tui: Tui;
  history: History;
  handles: { chrome: ChromeHandles; cardSlot: ViewSlot; pane: ScrollPane };
  state: ProbeState;
  lastBodyKey?: string;
  oracle: ManualChromeOracle;
}

async function openArmSession(): Promise<ArmContext> {
  const tui = await Tui.open({ width: WIDTH, height: HEIGHT, headless: true });
  const composer = tui.createTextInput({ multiline: true });
  const workingSlot = tui.createViewSlot(View.spacer(0));
  const cardSlot = tui.createViewSlot(View.vertical([View.text("\u25CF bash \u2014 preparing").fillWidth()]).fillWidth());
  const pane = tui.createScrollPane(View.spacer(0));
  const history = tui.createHistory();
  history.setLayout({ padding: 1, gap: 1 });
  return {
    tui,
    history,
    handles: { chrome: { composer, workingSlot }, cardSlot, pane },
    state: initialState(),
    oracle: new ManualChromeOracle(),
  };
}

/** One arm-specific scene render/update cycle for the current state. */
function runSceneOp(arm: string, ctx: ArmContext): void {
  const { tui, history, handles, state } = ctx;
  switch (arm) {
    case "current_body_key": {
      const key = bodyKey(state);
      if (key === ctx.lastBodyKey) {
        tui.advance(0);
        return;
      }
      ctx.lastBodyKey = key;
      tui.render(new Scene(buildChromeFresh(state, handles.chrome), history));
      return;
    }
    case "rebuild_uncomposed":
      tui.render(new Scene(buildChromeFresh(state, handles.chrome), history));
      return;
    case "manual_stable_oracle":
      tui.render(new Scene(ctx.oracle.body(state, handles.chrome), history));
      return;
    default:
      throw new Error(`unknown arm ${arm}`);
  }
}

// --- Statistics / provenance helpers (PERF-12 conventions). ----------------

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

// --- Measurement. -----------------------------------------------------------

async function measureArm(arm: string): Promise<{ screens: Map<string, string>; records: unknown[] }> {
  const ctx = await openArmSession();
  const records: unknown[] = [];
  const screens = new Map<string, string>();
  try {
    for (const entry of CASES) {
      const { state, handles } = ctx;
      // Warmup ops establish retained roots/caches for this case's shape.
      for (let op = 0; op < WARMUP; op += 1) {
        entry.advance(state, WARMUP + op);
        entry.boundaryOp?.(WARMUP + op, handles);
        runSceneOp(arm, ctx);
      }
      resetRetainedIdentityCounters();
      const before = retainedIdentityCounterSnapshot();
      const samples: number[] = [];
      for (let op = 0; op < MEASURED; op += 1) {
        entry.advance(state, op);
        entry.boundaryOp?.(op, handles);
        const started = Bun.nanoseconds();
        runSceneOp(arm, ctx);
        samples.push(Number(Bun.nanoseconds() - started));
      }
      const after = retainedIdentityCounterSnapshot();
      records.push({
        record_kind: "t13_1_step1_arm_case",
        profile: "smoke",
        benchmark_version: "PERF-12",
        tranche: "T13.1",
        step: 1,
        arm,
        case: entry.name,
        warmup_ops: WARMUP,
        measured_ops: MEASURED,
        samples_ns: samples.map(Math.round),
        median_ns: Math.round(median(samples)),
        p95_ns: Math.round(percentile(samples, 0.95)),
        p99_ns: Math.round(percentile(samples, 0.99)),
        median_ci95_ns: bootstrapMedianCi95(samples).map(Math.round),
        structural_delta: counterDelta(before, after),
      });
      screens.set(entry.name, screenSignature(ctx.tui));
    }
  } finally {
    ctx.tui.close();
  }
  return { screens, records };
}

function screenSignature(tui: Tui): string {
  return tui.screenRows().map((row) => row.replace(/\s+$/, "")).join("\n");
}

// --- Main. -------------------------------------------------------------------

const ARMS = ["current_body_key", "rebuild_uncomposed", "manual_stable_oracle"] as const;

const results = new Map<string, { screens: Map<string, string>; records: unknown[] }>();
for (const arm of ARMS) {
  results.set(arm, await measureArm(arm));
}

// Cross-arm parity: identical deterministic state sequences must end each
// case on identical screens (visible-semantics baseline, handoff §32.4).
const referenceScreens = results.get("current_body_key")!.screens;
for (const [caseName, reference] of referenceScreens) {
  for (const arm of ARMS) {
    const other = results.get(arm)!.screens.get(caseName)!;
    if (other !== reference) {
      throw new Error(`screen parity failure in case ${caseName} between current_body_key and ${arm}`);
    }
  }
}

const provenance = {
  record_kind: "t13_1_step1_provenance",
  profile: "smoke",
  benchmark_version: "PERF-12",
  tranche: "T13.1",
  step: 1,
  git_sha: commandText(["git", "rev-parse", "HEAD"]),
  perf7v2_sha: "e5292d62c4011610850cbdc1ba4a35f296f78e4f",
  perf11v4_result_sha: "7c670ccd99fb296b18719f62c1aa845a3e3605de",
  bun_version: Bun.version,
  bun_revision: commandText(["bun", "--revision"]),
  rustc_version: commandText(["rustc", "--version"]),
  target: commandText(["rustc", "-vV"]).split("host: ")[1]?.split("\n")[0] ?? "unknown",
  addon_sha256: commandText(["shasum", "-a", "256", "packages/iyon-runtime/native/iyon-native.node"]).split(" ")[0],
  note: "Step 1 baseline captured BEFORE the composition runtime exists (§48 order). The composed_auto arm reuses this harness once T13.1 lands. Arms run sequentially in one process; counter deltas isolate per-case windows.",
};

const artifact = [JSON.stringify(provenance), ...ARMS.flatMap((arm) => results.get(arm)!.records)]
  .map((record) => JSON.stringify(record))
  .join("\n") + "\n";
writeFileSync("packages/iyon-runtime/bench/PERF-12-T13.1-composition-baseline.jsonl", artifact);

// Compact console summary.
for (const arm of ARMS) {
  for (const record of results.get(arm)!.records as Array<{ case: string; median_ns: number; structural_delta: Counters }>) {
    console.log(
      `${arm.padEnd(20)} ${record.case.padEnd(20)} median ${String(record.median_ns).padStart(8)} ns  `
      + `mat=${record.structural_delta.direct_materializer_calls} hints=${record.structural_delta.bridge_hint_hits} `
      + `host=${record.structural_delta.host_mutations} fallbacks=${record.structural_delta.cold_fallbacks}`,
    );
  }
}
