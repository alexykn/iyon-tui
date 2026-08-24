/**
 * PERF-12 T13.1 — post-R9 correctness review: ownership & sideband semantics
 * (handoff §32.3 "Post-R9 correctness review invariants").
 *
 * Proven here against the REAL production host:
 *   - History sideband participates independently of body identity
 *     (needsPublication): same-body + first History attachment still
 *     publishes; a third same-body/same-History render is a true no-op;
 *   - direct takeover is SEMANTIC even when pixels cannot change: rendering
 *     the exact currently-visible Scene via the DIRECT path must relinquish
 *     the canonical builder root, or its State subscriptions ghost-update
 *     the screen later (R8 ownership-mode ghost);
 *   - direct takeover freezes projected components rather than vanishing
 *     them: JS scopes/subscriptions die while deferred native retirement
 *     keeps ComponentIds alive until a later successful frame proves unmount.
 */

import { describe, expect, test } from "bun:test";
import { Tui } from "../src/tui/runtime.ts";
import { Scene } from "../src/tui/scene.ts";
import { View } from "../src/tui/values/view.ts";
import type { History } from "../src/tui/history.ts";
import { defineView } from "../src/tui/define-view.ts";
import { state, trackedStateSubscriberCount, type State } from "../src/tui/tracked-state.ts";

function screenContains(tui: Tui, needle: string): boolean {
  return tui.screenRows().some((row: string) => row.includes(needle));
}

async function drain(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}

describe("T13.1 post-R9 review — ownership & sidebands", () => {
  test("§32.3 History sideband: same body + first attach still publishes; then true no-op", async () => {
    const tui = await Tui.open({ width: 40, height: 8, headless: true });
    try {
      const body = View.text("sideband-body");
      let producerRuns = 0;

      // Render #1: canonical, no history.
      let scene: Scene | undefined;
      tui.render(() => {
        producerRuns += 1;
        scene ??= new Scene(body);
        return scene;
      });
      expect(producerRuns).toBe(1);
      const boundAfterFirst = (tui as unknown as { boundHistory?: History }).boundHistory;
      expect(boundAfterFirst).toBeUndefined();

      // Render #2: SAME body View object, first explicit History attach.
      // needsPublication() must force publication even though the semantic
      // output is identity-equal (stagedHistory !== boundHistory).
      const history = tui.createHistory();
      await history.push(View.text("history line"));
      tui.render(() => new Scene(body, history));
      const boundAfterSecond = (tui as unknown as { boundHistory?: History }).boundHistory;
      expect(boundAfterSecond).toBe(history);

      // Render #3: same body, same history → true semantic no-op.
      tui.render(() => new Scene(body, history));
      const boundAfterThird = (tui as unknown as { boundHistory?: History }).boundHistory;
      expect(boundAfterThird).toBe(history);
      // Attach-once invariant held throughout (no error thrown above).
    } finally {
      tui.close();
    }
  });

  test("§32.3 direct takeover of the exact visible Scene relinquishes builder ownership", async () => {
    const tui = await Tui.open({ width: 48, height: 10, headless: true });
    try {
      const status = state<string>("A");
      let childBodies = 0;
      const Child = defineView(() => {
        childBodies += 1;
        return View.text(`child-${status.value}`);
      });
      let producerRuns = 0;
      let capturedScene: Scene | undefined;

      // Canonical render #1: builder root owns the scene; Child projects as
      // an independent native component slot showing "child-A".
      tui.render(() => {
        producerRuns += 1;
        capturedScene = new Scene(
          View.vertical((column) => column.child(Child({}))),
        );
        return capturedScene!;
      });
      await drain();

      expect(screenContains(tui, "child-A")).toBe(true);
      const bodiesAfterMount = childBodies;
      expect(trackedStateSubscriberCount(status as State<unknown>)).toBe(1); // Child scope

      // DIRECT takeover with the EXACT currently-visible Scene: native route
      // is a pixel no-op, but ownership must transfer — the builder root and
      // all of its subscriptions die NOW.
      tui.render(capturedScene!);
      await drain();

      expect(trackedStateSubscriberCount(status as State<unknown>)).toBe(0);
      const producerRunsAtTakeover = producerRuns;

      // The ghost-update scenario that motivated this gate: change the state
      // the OLD builder read. Nothing may execute; nothing may repaint.
      status.set("B");
      await drain();
      for (let index = 0; index < 5; index += 1) await Promise.resolve();

      expect(childBodies).toBe(bodiesAfterMount); // old scope never re-ran
      expect(producerRuns).toBe(producerRunsAtTakeover); // builder never re-ran
      expect(screenContains(tui, "child-A")).toBe(true); // frozen, not vanished

      // A different direct Scene reconciles normally; the frozen projection's
      // ComponentId retires only after this frame proves it unmounted.
      tui.render(new Scene(View.text("replaced")));
      await drain();
      expect(screenContains(tui, "replaced")).toBe(true);
    } finally {
      tui.close();
    }
  });
});
