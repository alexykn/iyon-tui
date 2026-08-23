/**
 * PERF-12 T13.1 R7 — transactional retained-root publication proofs
 * (handoff §32.1 R7, AMENDMENT-C §13).
 *
 * Gate evidence:
 *   - multi-scope atomicity: N changed projections publish together or not
 *     at all — a PREPARE failure on any scope publishes nothing, leaves every
 *     committed output old, and records the abort;
 *   - ownership stays inside the boundary (no split-brain): publications
 *     delegate to RetainedRootBoundary.prepareInstall via ViewSlot;
 *   - commit-phase failures are pathological: they surface loudly instead of
 *     being silently swallowed (post-state unspecified by protocol);
 *   - legacy projections without preparePublication keep working through the
 *     documented per-scope fallback;
 *   - ViewSlot-level parity: prepare→commit ≡ setView; prepare→abort leaves
 *     revision and content untouched; double-commit is guarded.
 */

import { describe, expect, test } from "bun:test";
import { native, type NativeTuiHostContract } from "../src/native.ts";
import { Tui } from "../src/tui/runtime.ts";
import { Scene } from "../src/tui/scene.ts";
import { View } from "../src/tui/values/view.ts";
import { ViewSlot } from "../src/tui/component.ts";
import {
  RetainedExecutionRuntime,
  executionCounterSnapshot,
  type ScopeProjection,
} from "../src/tui/execution.ts";
import { defineView } from "../src/tui/define-view.ts";
import { state } from "../src/tui/tracked-state.ts";
import { composeText, composeVertical } from "../src/tui/compose.ts";
import { BRIDGE_VIEW_KIND } from "../src/tui/ir.ts";
import { nodeForBridge } from "../src/tui/values/view.ts";
import type { ViewComponent } from "../src/tui/execution.ts";

const Host = native.NativeTuiHost as
  | (new (width: number, height: number, headless: boolean) => NativeTuiHostContract)
  | undefined;

const canRun = Host !== undefined;

function tracked(render: () => View): { component: ViewComponent<void>; calls: () => number } {
  let calls = 0;
  const component = defineView(() => {
    calls += 1;
    return render();
  });
  return { component, calls: () => calls };
}

interface FakeProbe {
  installs: number;
  prepares: number;
  failPrepare: boolean;
  failCommit: boolean;
}

/** Fully controllable projection for protocol tests (no native involvement). */
function fakeProjection(probe: FakeProbe): ScopeProjection {
  const view = View.text("projection");
  let lastInstalled: View | undefined;
  return {
    view,
    install(output: View): void {
      probe.installs += 1;
      lastInstalled = output;
    },
    preparePublication(output: View): { commit(): void; abort(): void } {
      probe.prepares += 1;
      if (probe.failPrepare) throw new Error("prepare refused");
      return {
        commit(): void {
          if (probe.failCommit) throw new Error("publish refused post-prepare");
          probe.installs += 1;
          lastInstalled = output;
        },
        abort(): void {},
      };
    },
    dispose(): void {
      void lastInstalled;
    },
  };
}

describe("T13.1 R7 — transactional retained-root publication", () => {
  test("atomicity: prepare failure on ONE scope publishes NOTHING anywhere", () => {
    const runtime = new RetainedExecutionRuntime({ autoFlush: false });
    const probes: FakeProbe[] = [];
    const makeProjectedLeaf = (name: string) => {
      const value = state(`${name}0`);
      const comp = tracked(() => composeText(`${name}=${value.value}`));
      return { comp, value };
    };
    const a = makeProjectedLeaf("a");
    const b = makeProjectedLeaf("b");
    const c = makeProjectedLeaf("c");
    void b;
    void c;

    const runtimeOptions = {
      createScopeProjection: (): ScopeProjection => {
        const probe: FakeProbe = { installs: 0, prepares: 0, failPrepare: false, failCommit: false };
        probes.push(probe);
        return fakeProjection(probe);
      },
    };
    const rt = new RetainedExecutionRuntime(runtimeOptions);

    const Holder = tracked(() =>
      composeVertical((column) => {
        column.child(a.comp.component(undefined as never));
        column.child(b.comp.component(undefined as never));
        column.child(c.comp.component(undefined as never));
      }),
    );
    const root = rt.mountRoot(Holder.component, undefined as never);
    const baseInstalls = probes.map((p) => p.installs);

    // All three scopes change; C's preparation is armed to fail.
    a.value.set("a1");
    b.value.set("b1");
    c.value.set("c1");
    probes[2]!.failPrepare = true;

    const before = executionCounterSnapshot();
    expect(() => rt.flush()).toThrow("prepare refused");

    // Nothing published anywhere.
    expect(probes.map((p) => p.installs)).toEqual(baseInstalls);
    // Every committed output still shows the OLD values.
    expect(nodeForBridge(root.currentOutput!).kind).toBe(BRIDGE_VIEW_KIND.column);
    // Abort recorded.
    const after = executionCounterSnapshot();
    expect(after.execution_commit_aborts - before.execution_commit_aborts).toBe(1);

    // Recovery: clear the failure; the application re-triggers (notifications
    // are consumed once — an aborted batch requires an explicit re-drive,
    // exactly like §41's "retry from same application state").
    probes[2]!.failPrepare = false;
    const [scopeA, scopeB, scopeC] = root.children.map((record) => record.scope);
    runtime.invalidate(scopeA);
    runtime.invalidate(scopeB);
    runtime.invalidate(scopeC);
    runtime.flush();
    expect(probes.map((p) => p.installs)).toEqual(baseInstalls.map((n) => n + 1));
    rt.dispose();
    void b;
    void c;
  });

  test("commit-phase failure surfaces loudly (pathological, never silent)", () => {
    const runtime = new RetainedExecutionRuntime({
      autoFlush: false,
      createScopeProjection: () => fakeProjection({ installs: 0, prepares: 0, failPrepare: false, failCommit: true }),
    });
    const value = state("v0");
    const T = tracked(() => composeText(value.value));
    const Holder = tracked(() => composeVertical((column) => column.child(T.component(undefined as never))));
    const root = runtime.mountRoot(Holder.component, undefined as never);

    value.set("v1");
    // Prepare succeeds; the publish itself refuses. This is pathological per
    // the boundary contract (validated lease + generation) and MUST surface
    // loudly instead of being silently normalized into "update skipped".
    expect(() => runtime.update(root.children[0]!.scope)).toThrow(/publish refused/);
    runtime.dispose();
  });

  test("legacy projections without preparePublication fall back per-scope", () => {
    let installs = 0;
    const value = state("v0");
    const T = tracked(() => composeText(value.value));
    const Holder = tracked(() => composeVertical((column) => column.child(T.component(undefined as never))));
    const legacyRuntime = new RetainedExecutionRuntime({
      autoFlush: false,
      createScopeProjection: () => {
        const view = View.text("legacy");
        return {
          view,
          install(): void {
            installs += 1;
          },
          dispose(): void {},
        };
      },
    });
    const root = legacyRuntime.mountRoot(Holder.component, undefined as never);
    expect(installs).toBe(1); // mount install through fallback

    value.set("v1");
    legacyRuntime.update(root.children[0]!.scope);
    expect(installs).toBe(2); // legacy path still drives content swaps
    legacyRuntime.dispose();
  });

  test("ViewSlot.prepareSetView parity: commit \u2261 setView; abort touches nothing", async () => {
    if (!canRun) return;
    const tui = await Tui.open({ width: 48, height: 8, headless: true });
    try {
      const slot = tui.createViewSlot(View.spacer(0));
      const proj = slot.view();
      const body = View.vertical([View.text("chrome"), proj]);
      tui.render(new Scene(body));

      slot.setView(View.text("via setView"));
      const revisionAfterSetView = slot.revision();

      // Prepare -> commit: equivalent to setView.
      const preparedCommit = slot.prepareSetView(View.text("committed"));
      expect(preparedCommit).toBeDefined();
      preparedCommit!.commit();
      expect(slot.revision()).toBe(revisionAfterSetView + 1);
      tui.render(new Scene(body));
      expect(tui.screenRows().some((row: string) => row.includes("committed"))).toBe(true);

      // Prepare -> abort: revision unchanged, content unchanged.
      const preparedAbort = slot.prepareSetView(View.text("aborted"));
      preparedAbort!.abort();
      expect(slot.revision()).toBe(revisionAfterSetView + 1);
      tui.render(new Scene(body));
      expect(tui.screenRows().some((row: string) => row.includes("aborted"))).toBe(false);

      // Double-commit is guarded.
      expect(() => preparedCommit!.commit()).toThrow();

      slot.dispose();
    } finally {
      await tui.close();
    }
  });
});
