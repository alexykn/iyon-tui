/**
 * PERF-12 T13 (§49/§77–§81, §114, §115): production boundary routing.
 *
 * Every View-bearing boundary the production trace identified — scene root,
 * History units, ViewSlots (tool cards / spinner), ScrollPanes (tool output),
 * animations, and theme installation — now routes through retained semantic
 * identity. This suite proves:
 *
 *   - remaining-kind materializers (container/clamp/contentMax/hanging/
 *     decorated/component) render identically to the Direct oracle;
 *   - no boundary silently falls back on retained traces (counters);
 *   - §114 dormant-node recovery and generation-reset behavior;
 *   - §115 multi-host lease correctness over one shared semantic root;
 *   - §81 animation cycles create zero fallbacks after the first frame set;
 *   - §78 history unit import reuses shared identity across boundaries.
 */

import { describe, expect, test } from "bun:test";

import { native } from "../src/native.ts";
import {
  nativeViewAbiSession,
  tryNativeMaterialize,
} from "../src/tui/native_view_abi.ts";
import {
  peekBridgeNativeHint,
  resetRetainedIdentityCounters,
  retainedIdentityCounterSnapshot,
  RetainedRootBoundary,
  forceBridgeNativeHintForTests,
  type RetainedIdentityCounters,
} from "../src/tui/retained_dag.ts";
import { nodeForBridge, View } from "../src/tui/values/view.ts";
import { Style } from "../src/tui/values/style.ts";
import { Insets } from "../src/tui/values/geometry.ts";
import { Tui } from "../src/tui/runtime.ts";
import { TextStream } from "../src/tui/stream.ts";

const Host = native.NativeTuiHost as
  | (new (width: number, height: number, headless: boolean) => {
    render(node: object): void;
    screenRows(): string[];
    dispose(): void;
    tuiViewAbiHostPointer?(): number;
  })
  | undefined;

const session = nativeViewAbiSession();
const canRun = Host !== undefined && session !== undefined;

function countersDelta(action: () => void): RetainedIdentityCounters {
  const before = retainedIdentityCounterSnapshot();
  action();
  const after = retainedIdentityCounterSnapshot();
  const delta = {} as Record<string, number>;
  for (const key of Object.keys(after) as (keyof RetainedIdentityCounters)[]) {
    delta[key] = (after[key] as number) - (before[key] as number);
  }
  return delta as unknown as RetainedIdentityCounters;
}

function directOracle(view: View, width = 48, height = 14): string[] {
  const host = new Host!(width, height, true);
  try {
    host.render(nodeForBridge(view));
    return host.screenRows();
  } finally {
    host.dispose();
  }
}

describe("PERF-12 T13 boundary routing", () => {
  test("§76: remaining kinds install through ensureNative with Direct parity", () => {
    if (!canRun) return;
    const inner = View.vertical([
      View.text("alpha").fillWidth(),
      View.hanging(
        View.text("* ").noWrap(),
        View.text("  ").noWrap(),
        View.text("hanging body text wraps across the terminal width here").fillWidth(),
      ).fillWidth(),
    ]);
    const cases: [string, View][] = [
      ["container", inner.container()],
      ["clamp-footer", View.vertical(Array.from({ length: 24 }, (_, i) => View.text(`row ${i}`)))
        .clampRows(5, { kind: "footer", prefix: "… more lines", style: Style.new().foreground("theme:text.muted").italic().dim() })],
      ["clamp-ellipsis", View.vertical(Array.from({ length: 24 }, (_, i) => View.text(`row ${i}`)))
        .clampRows(4, { kind: "ellipsis", style: Style.new().foreground("theme:truncation_footer") })],
      ["contentMax", View.contentMax(3, View.vertical(Array.from({ length: 16 }, (_, i) => View.text(`c${i}`))))],
      // The exact production tool-card shape: hanging bullet line + hanging
      // result line + collapseResultView clamp, all theme-keyed styles.
      ["tool-card", View.vertical([
        View.hanging(
          View.text("\u25CF ").style(Style.new().foreground("theme:tool.finished")).noWrap(),
          View.text("  ").noWrap(),
          View.text("bash \u2014 finished").style(Style.new().foreground("theme:tool.finished")).fillWidth(),
        ).fillWidth(),
        View.hanging(
          View.text("  ").noWrap(),
          View.text("  ").noWrap(),
          View.text("exit 0").style(Style.new().foreground("theme:text.muted")).fillWidth(),
        ).fillWidth(),
        View.hanging(
          View.text("  ").noWrap(),
          View.text("  ").noWrap(),
          View.text("$ ls -la").style(Style.new().foreground("theme:text.muted")).fillWidth(),
        ).fillWidth(),
      ]).clampRows(16, { kind: "footer", prefix: "\u2026 more lines (full result retained)", style: Style.new().foreground("theme:truncation_footer").italic().dim() })],
      ["decorated-full", View.horizontal([View.spacer(1), View.text("styled")])
        .padding(Insets.of(1, 2, 1, 2))
        .foreground("#ff8000")
        .background("theme:surface.user")
        .border({ style: "rounded", edges: "all", color: "green" })
        .style(Style.new().theme("text.muted").bold())
        .styleState("iyon.agent.effort", "high")
        .fillWidth()
        .minHeight(2)
        .maxHeight(8)],
    ];
    for (const [name, view] of cases) {
      const host = new Host!(48, 14, true);
      const boundary = new RetainedRootBoundary(session!, () => host.tuiViewAbiHostPointer?.()! as never);
      try {
        const delta = countersDelta(() => expect(boundary.install(view)).toBeGreaterThan(0));
        expect(delta.cold_fallbacks).toBe(0);
        expect(host.screenRows()).toEqual(directOracle(view, 48, 14));
        void name;
      } finally {
        boundary.close();
        host.dispose();
      }
    }
  });

  test("§76: component references materialize through registered native slots", async () => {
    if (!canRun) return;
    const runtime = await Tui.open({ width: 48, height: 14, headless: true });
    try {
      const slot = runtime.createViewSlot(View.spacer(2));
      const tree = View.vertical([View.text("inside"), View.component(slot).fillWidth()]);
      // Render through the SAME runtime whose host registered the component.
      resetRetainedIdentityCounters();
      runtime.render(new SceneLike(tree));
      const c = retainedIdentityCounterSnapshot();
      expect(c.cold_fallbacks).toBe(0);
      // The registered slot's content paints (a Direct oracle on a different
      // host cannot resolve this component id — registration is per-host).
      expect(runtime.screenRows().some((row) => row.includes("inside"))).toBe(true);
      slot.dispose();
    } finally {
      await runtime.close();
    }
  });

  test("B1: scene renders ride exact-root then retained lanes with zero fallbacks", async () => {
    if (!canRun) return;
    const runtime = await Tui.open({ width: 48, height: 14, headless: true });
    try {
      const composerSlot = runtime.createViewSlot(View.spacer(1));
      const buildBody = (footer: string) =>
        View.vertical((column) => {
          column.child(View.component(composerSlot).fillWidth());
          column.child(View.text(footer).fillWidth());
        }).fillWidth();

      resetRetainedIdentityCounters();
      runtime.render(new SceneLike(buildBody("v1")));
      const first = retainedIdentityCounterSnapshot();
      expect(first.cold_fallbacks + first.direct_materializer_calls).toBeGreaterThan(0);
      expect(first.cold_fallbacks).toBe(0);

      // Identical body object → no-op identity cutoff above the bridge.
      const sameBody = (runtime as unknown as { current(): { body: View } }).current()!.body;
      const before = retainedIdentityCounterSnapshot();
      runtime.render(new SceneLike(sameBody));
      expect(retainedIdentityCounterSnapshot().host_mutations).toBe(before.host_mutations);

      // Changed frontier (new footer text only) → small retained install.
      runtime.render(new SceneLike(buildBody("v2 — footer changed")));
      const second = retainedIdentityCounterSnapshot();
      expect(second.cold_fallbacks).toBe(first.cold_fallbacks);
      expect(runtime.screenRows().some((row) => row.includes("v2"))).toBe(true);

      // Warm exact root → exactly one host mutation per render.
      const mutationsBefore = retainedIdentityCounterSnapshot().host_mutations;
      const body3 = buildBody("v2 — footer changed");
      runtime.render(new SceneLike(body3));
      runtime.render(new SceneLike(body3));
      expect(retainedIdentityCounterSnapshot().host_mutations - mutationsBefore).toBe(1);
      composerSlot.dispose();
    } finally {
      await runtime.close();
    }
  });

  test("B3/B4: slot and pane updates reuse identity without rebuilding the old tree", async () => {
    if (!canRun) return;
    const runtime = await Tui.open({ width: 48, height: 14, headless: true });
    try {
      const slot = runtime.createViewSlot(View.spacer(0));
      const pane = runtime.createScrollPane(View.spacer(0));

      resetRetainedIdentityCounters();
      slot.setView(View.vertical([View.text("card line one"), View.text("arg preview")]));
      pane.setContent(View.vertical([View.text("out 1"), View.text("out 2")]).fillWidth());
      const first = retainedIdentityCounterSnapshot();
      expect(first.cold_fallbacks).toBe(0);

      // Update both repeatedly: every update must be hint-driven (no fallback,
      // no N-API bridge). The pre-T13 path rebuilt the PREVIOUS content cold on
      // every single update; the counter below proves that work is gone.
      for (let i = 0; i < 8; i += 1) {
        slot.setView(View.vertical([View.text(`card ${i}`), View.text(`args ${i}`)]));
        pane.setContent(View.vertical([View.text(`out ${i}.a`), View.text(`out ${i}.b`)]).fillWidth());
        pane.followEnd();
      }
      const last = retainedIdentityCounterSnapshot();
      expect(last.cold_fallbacks).toBe(0);
      expect(last.host_mutations).toBe(first.host_mutations); // slots are not scene hosts

      // Content actually updated: repaint a scene that references both
      // components (slot damage becomes visible on the next scene render).
      const reveal = View.vertical([
        View.component(slot).fillWidth(),
        View.component(pane).fillWidth(),
      ]);
      runtime.render(new SceneLike(reveal));
      expect(runtime.screenRows().some((r) => r.includes("card 7"))).toBe(true);
      expect(runtime.screenRows().some((r) => r.includes("out 7.b"))).toBe(true);
      slot.dispose();
      pane.dispose();
    } finally {
      await runtime.close();
    }
  });

  test("§81: animation cycles stop creating materializer calls after warm-up", async () => {
    if (!canRun) return;
    const runtime = await Tui.open({ width: 48, height: 14, headless: true });
    try {
      const spinner = runtime.createViewSlot(View.spacer(0));
      const frames = [
        View.text("(working)").noWrap(),
        View.text("-working-").noWrap(),
        View.text("\\working/").noWrap(),
      ];
      resetRetainedIdentityCounters();
      spinner.setAnimation(frames, 80);
      const warm = retainedIdentityCounterSnapshot();
      expect(warm.cold_fallbacks).toBe(0);
      const materializersWarm = warm.direct_materializer_calls;
      expect(materializersWarm).toBeGreaterThan(0);

      // Re-setting the SAME frame views must hit hints only: stable frame
      // objects are the animation's semantic identity (§81: no per-frame
      // full-tree bridge).
      for (let cycle = 0; cycle < 4; cycle += 1) {
        spinner.setAnimation(frames, 80);
      }
      const after = retainedIdentityCounterSnapshot();
      expect(after.direct_materializer_calls).toBe(materializersWarm);
      expect(after.cold_fallbacks).toBe(0);
      spinner.stopAnimation(frames[0]!);
      expect(retainedIdentityCounterSnapshot().cold_fallbacks).toBe(0);
      spinner.dispose();
    } finally {
      await runtime.close();
    }
  });

  test("§78: history unit import reuses shared identity across boundaries", async () => {
    if (!canRun) return;
    const runtime = await Tui.open({ width: 48, height: 14, headless: true });
    const history = runtime.createHistory();
    try {
      const shared = View.vertical([View.text("shared card shell"), View.text("stable body")]);
      // Materialize once via a slot boundary (the tool-card shape).
      const slot = runtime.createViewSlot(shared);
      // Pushing the SAME View rides its hint: zero new materializations.
      const delta = countersDelta(() => history.push(shared.fillWidth()));
      expect(delta.direct_materializer_calls).toBe(0);
      expect(delta.cold_fallbacks).toBe(0);
      // A fresh unit still imports cleanly alongside it.
      expect(history.push(View.text("fresh unit"))).toBeGreaterThan(0);
      slot.dispose();
      history.dispose();
    } finally {
      await runtime.close();
    }
  });

  test("§114: dormant node recovers through rematerialization after native expiry", () => {
    if (!canRun) return;
    // Seed S natively through a Direct decode, then evict everything.
    const dormant = View.vertical([View.text("dormant payload"), View.text("second line")]);
    const seeder = new Host!(48, 14, true);
    seeder.render(nodeForBridge(dormant));
    seeder.dispose();

    const maintainer = new Host!(48, 14, true);
    try {
      const maintain = native.tuiViewAbiMaintain;
      if (maintain !== undefined) maintain(true);
      const snapshot = native.tuiViewRuntimeMemorySnapshot?.(false);
      void snapshot;

      // Reinsert into a NEW parent and install. The stale/expired state must
      // resolve in ONE bounded step: promotion miss → fresh materialization,
      // correct render, no persistent mirror, no crash.
      const parent = View.horizontal([View.spacer(1), dormant]);
      const host = new Host!(48, 14, true);
      const boundary = new RetainedRootBoundary(session!, () => host.tuiViewAbiHostPointer?.()! as never);
      try {
        const ref = boundary.install(parent);
        expect(ref).toBeGreaterThan(0);
        expect(host.screenRows()).toEqual(directOracle(parent));
        // The recovered subtree got fresh hints under the current generation.
        expect(peekBridgeNativeHint(nodeForBridge(dormant))?.generation).toBe(session!.abi.generation);
      } finally {
        boundary.close();
        host.dispose();
      }
    } finally {
      maintainer.dispose();
    }
  });

  test("§114b: poisoned stale hint on a dormant root triggers exactly one recovery", () => {
    if (!canRun) return;
    const root = View.vertical([View.text("stale root payload")]);
    const seeder = new Host!(48, 14, true);
    seeder.render(nodeForBridge(root));
    seeder.dispose();

    const host = new Host!(48, 14, true);
    const boundary = new RetainedRootBoundary(session!, () => host.tuiViewAbiHostPointer?.()! as never);
    try {
      // Force a generation-valid but dead hint: the §47 targeted retry must
      // drop it, promote or rematerialize once, and still render correctly.
      forceBridgeNativeHintForTests(nodeForBridge(root), {
        generation: session!.abi.generation,
        nativeRef: 0x7fff_fe10,
      });
      resetRetainedIdentityCounters();
      const ref = boundary.install(root);
      expect(ref).toBeGreaterThan(0);
      const c = retainedIdentityCounterSnapshot();
      expect(c.stale_ref_retries).toBe(1);
      expect(host.screenRows()).toEqual(directOracle(root));
    } finally {
      boundary.close();
      host.dispose();
    }
  });

  test("§115: two hosts share one semantic root with independent leases", () => {
    if (!canRun) return;
    const sharedRoot = View.vertical([
      View.text("multi host root"),
      View.horizontal([View.spacer(1), View.text("branch")]),
    ]);
    const hostA = new Host!(48, 14, true);
    const hostB = new Host!(48, 14, true);
    const boundaryA = new RetainedRootBoundary(session!, () => hostA.tuiViewAbiHostPointer?.()! as never);
    const boundaryB = new RetainedRootBoundary(session!, () => hostB.tuiViewAbiHostPointer?.()! as never);
    try {
      const leasedBefore = native.tuiViewRuntimeMemorySnapshot?.(true)?.leased_slots;
      const refA = boundaryA.install(sharedRoot);
      const refB = boundaryB.install(sharedRoot);
      expect(refA).toBeGreaterThan(0);
      expect(refB).toBeGreaterThan(0);
      // Same semantic NodeId resolves to ONE NativeRef; each boundary holds
      // its own lease on that shared slot (T6 finding-1 ownership rule).
      expect(refB).toBe(refA);
      expect(hostA.screenRows()).toEqual(directOracle(sharedRoot));
      expect(hostB.screenRows()).toEqual(directOracle(sharedRoot));

      // A replaces its root; B keeps rendering the original exactly.
      const replacement = View.vertical([View.text("replacement root")]);
      expect(boundaryA.install(replacement)).toBeGreaterThan(0);
      expect(hostA.screenRows()).toEqual(directOracle(replacement));
      expect(boundaryB.renderExact(sharedRoot).status).toBe("ok");
      expect(hostB.screenRows()).toEqual(directOracle(sharedRoot));

      boundaryA.close();
      boundaryB.close();
      // Both boundary leases drained: leased slots return to the entry level.
      const leasedAfter = native.tuiViewRuntimeMemorySnapshot?.(true)?.leased_slots;
      expect(leasedAfter ?? 0).toBe(leasedBefore ?? 0);
    } finally {
      hostA.dispose();
      hostB.dispose();
    }
  });

  test("§49: an oversized cold tree aborts the retained prefix and still renders", () => {
    if (!canRun) return;
    // > MAX_RETAINED_NEW_NODES leaves: the budget abort fires mid-tree, the
    // Direct decode completes the render, and published prefix nodes make the
    // decode cheaper instead of being rolled back.
    const leaves = Array.from({ length: 700 }, (_, i) => View.text(`leaf-${i}`));
    const big = View.vertical(leaves);
    const host = new Host!(64, 20, true);
    const boundary = new RetainedRootBoundary(session!, () => host.tuiViewAbiHostPointer?.()! as never);
    try {
      resetRetainedIdentityCounters();
      const retainedRef = boundary.install(big);
      expect(retainedRef).toBeUndefined(); // budget refusal routed the cold path
      const c = retainedIdentityCounterSnapshot();
      expect(c.cold_fallbacks).toBeGreaterThan(0);
      host.render(nodeForBridge(big));
      expect(host.screenRows()[0]).toContain("leaf-");
    } finally {
      boundary.close();
      host.dispose();
    }
  });

  test("stream separation still holds behind the new boundaries (§42 guard)", async () => {
    if (!canRun) return;
    const runtime = await Tui.open({ width: 48, height: 14, headless: true });
    try {
      const stream = new TextStream({ projector: "markdown" });
      const history = runtime.createHistory();
      resetRetainedIdentityCounters();
      history.pushStream(stream);
      stream.append("hello **world**\n\nsecond paragraph");
      stream.seal();
      history.sealStream(stream);
      const c = retainedIdentityCounterSnapshot();
      expect(c.direct_materializer_calls).toBe(0);
      expect(c.bridge_children_visited).toBe(0);
      expect(c.byte_payload_bytes).toBe(0);
      expect(c.ref_words_written).toBe(0);
      history.dispose();
      await runtime.close();
    } finally {
      void canRun;
    }
  });
});

/** Minimal scene wrapper used to exercise `Tui.render`. */
class SceneLike {
  readonly body: View;
  constructor(body: View) { this.body = body; }
}
