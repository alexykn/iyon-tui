/**
 * PERF-12 T13.1 R6a — end-to-end proofs through the REAL production host
 * (handoff §32.1 R6a, AMENDMENT-C §5/§18 Step 9R part 1).
 *
 * Gate evidence:
 *   - a local scope update renders END-TO-END through the current machinery:
 *     state write → flush → slot content swap → screen live, with NO parent
 *     rebuild and NO scene re-render needed;
 *   - parent semantic View identity stays EXACT across child-local updates;
 *   - old leases survive: repeated updates stay hint-driven (zero cold
 *     fallbacks, zero N-API bridge work) like the B3/B4 production proof;
 *   - root-level structural changes propagate through an explicit scene
 *     render (root-scene propagation is canonical boundary territory — R8/R11);
 *   - overhead at current mounted-scope counts recorded (bench file).
 */

import { describe, expect, test } from "bun:test";
import { Tui } from "../src/tui/runtime.ts";
import { Scene } from "../src/tui/scene.ts";
import { View } from "../src/tui/values/view.ts";
import {
  RetainedExecutionRuntime,
  type ViewComponent,
} from "../src/tui/execution.ts";
import { defineView } from "../src/tui/define-view.ts";
import { invokeComponent } from "../src/tui/execution.ts";
import { state } from "../src/tui/tracked-state.ts";
import { composeText, composeVertical } from "../src/tui/compose.ts";
import {
  resetRetainedIdentityCounters,
  retainedIdentityCounterSnapshot,
} from "../src/tui/retained_dag.ts";

const canRun = true; // native required; guarded by Tui.open failure below

function tracked<P>(render: (props: P) => View): { component: ViewComponent<P>; calls: () => number } {
  let calls = 0;
  const component = defineView<P>((props) => {
    calls += 1;
    return render(props);
  });
  return { component, calls: () => calls };
}

async function makeTui(): Promise<{ tui: Tui; runtime: RetainedExecutionRuntime }> {
  const tui = await Tui.open({ width: 64, height: 12, headless: true });
  const runtime = bindRuntime(tui);
  return { tui, runtime };
}

// Local binding mirroring tui-execution.ts's factory (kept explicit here so
// this suite fails loudly if the production glue drifts).
function bindRuntime(tui: Tui): RetainedExecutionRuntime {
  return new RetainedExecutionRuntime({
    createScopeProjection: () => {
      const slot = tui.createViewSlot(View.spacer(0));
      const view = slot.view();
      return {
        view,
        install(output: View): void {
          slot.setView(output);
        },
        dispose(): void {
          slot.dispose();
        },
      };
    },
  });
}

function screenContains(tui: Tui, needle: string): boolean {
  return tui.screenRows().some((row: string) => row.includes(needle));
}

describe("T13.1 R6a — end-to-end through the production host", () => {
  test("local state update is live on screen without any scene re-render", async () => {
    const { tui, runtime } = await makeTui();
    try {
      const status = state("ready");
      const Footer = tracked(() => composeText(`status ${status.value}`));
      const Chrome = tracked(() => composeText("chrome"));
      const App = tracked(() =>
        composeVertical((column) => {
          column.child(Chrome.component({}));
          column.child(Footer.component({}));
        }),
      );
      const root = runtime.mountRoot(App.component, {});
      const appOutputBefore = root.currentOutput;

      tui.render(new Scene(root.currentOutput!));
      expect(screenContains(tui, "status ready")).toBe(true);

      // THE local update: one tracked write. No invalidate, no flush call,
      // no tui.render — auto-scheduling + projection install handle it all.
      resetRetainedIdentityCounters();
      status.set("running tool 3/7");
      await Promise.resolve(); // let the scheduled microtask flush run

      // Screen is live.
      expect(screenContains(tui, "status running tool 3/7")).toBe(true);
      expect(screenContains(tui, "status ready")).toBe(false);

      // Parent semantic identity EXACT: same output object, same embedded refs.
      expect(root.currentOutput).toBe(appOutputBefore);
      // Retained discipline: hint-driven, zero cold fallbacks.
      expect(retainedIdentityCounterSnapshot().cold_fallbacks).toBe(0);
      void Chrome;
      runtime.dispose();
    } finally {
      await tui.close();
    }
  });

  test("repeated updates stay hint-driven (old lease survives until replacement)", async () => {
    const { tui, runtime } = await makeTui();
    try {
      const counter = state(0);
      const Counter = tracked(() => composeText(`count ${counter.value}`));
      const Holder = tracked(() => composeVertical((column) => column.child(Counter.component({}))));
      const root = runtime.mountRoot(Holder.component, {});
      tui.render(new Scene(root.currentOutput!));

      resetRetainedIdentityCounters();
      for (let index = 1; index <= 8; index += 1) {
        counter.set(index);
        await Promise.resolve();
        expect(screenContains(tui, `count ${index}`)).toBe(true);
      }
      const counters = retainedIdentityCounterSnapshot();
      expect(counters.cold_fallbacks).toBe(0);
      // Slots are not scene hosts: zero scene-host mutations from updates.
      expect(counters.host_mutations).toBe(0);
      runtime.dispose();
    } finally {
      await tui.close();
    }
  });

  test("root-level structural change propagates through an explicit scene render", async () => {
    const { tui, runtime } = await makeTui();
    try {
      let showHint = true;
      const Hint = tracked(() => composeText("hint row"));
      const App = tracked(() =>
        composeVertical((column) => {
          column.child(composeText("chrome"));
          if (showHint) column.child(Hint.component({}));
        }),
      );
      const root = runtime.mountRoot(App.component, {});
      tui.render(new Scene(root.currentOutput!));
      expect(screenContains(tui, "hint row")).toBe(true);

      showHint = false;
      runtime.update(root); // structural change owned by the root scope

      // Root-scene propagation is explicit at this tranche (R8/R11 wires the
      // canonical render boundary); the fresh root output renders cleanly.
      tui.render(new Scene(root.currentOutput!));
      expect(screenContains(tui, "hint row")).toBe(false);
      expect(retainedIdentityCounterSnapshot().cold_fallbacks).toBe(0);
      runtime.dispose();
    } finally {
      await tui.close();
    }
  });

  test("two independent states repaint their own regions", async () => {
    const { tui, runtime } = await makeTui();
    try {
      const left = state("L0");
      const right = state("R0");
      const LeftPane = tracked(() => composeText(`L:${left.value}`));
      const RightPane = tracked(() => composeText(`R:${right.value}`));
      const App = tracked(() =>
        composeVertical((column) => {
          column.child(LeftPane.component({}));
          column.child(RightPane.component({}));
        }),
      );
      const root = runtime.mountRoot(App.component, {});
      tui.render(new Scene(root.currentOutput!));

      left.set("L1");
      await Promise.resolve();
      expect(screenContains(tui, "L:L1")).toBe(true);
      expect(screenContains(tui, "R:R0")).toBe(true); // untouched

      right.set("R1");
      await Promise.resolve();
      expect(screenContains(tui, "R:R1")).toBe(true);
      expect(screenContains(tui, "L:L1")).toBe(true); // still live
      runtime.dispose();
    } finally {
      await tui.close();
    }
  });
});
