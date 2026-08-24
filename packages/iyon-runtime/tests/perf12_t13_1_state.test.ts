/**
 * PERF-12 T13.1 R4 — tracked `State<T>` invalidation proofs (handoff §32.1
 * R4, AMENDMENT-C §7/§18 Step 7R).
 *
 * Gate evidence:
 *   - purity enforcement: writes inside component bodies reject deterministically;
 *   - subscription lifecycle: deps dropped when no longer read (post-commit);
 *     aborted evaluations retain the COMMITTED set;
 *   - Object.is change discipline: same-value sets are silent no-ops;
 *   - batching: multiple writes coalesce into one flush pass;
 *   - §31.1 execution-frontier gate END-TO-END: App/A/B/C each with their own
 *     State; writing B's state ⇒ body executions App=0 A=0 B=1 C=0 by counters.
 */

import { describe, expect, test } from "bun:test";
import { native, type NativeTuiHostContract } from "../src/native.ts";
import {
  RetainedExecutionRuntime,
  executionCounterSnapshot,
  type ViewComponent,
} from "../src/tui/execution.ts";
import { defineView } from "../src/tui/define-view.ts";
import { invokeComponent } from "../src/tui/execution.ts";
import { state, trackedStateSubscriberCount, type State } from "../src/tui/tracked-state.ts";
import { composeText, composeVertical } from "../src/tui/compose.ts";
import { BRIDGE_VIEW_KIND } from "../src/tui/ir.ts";
import { View, nodeForBridge } from "../src/tui/values/view.ts";
import { ViewSlot } from "../src/tui/component.ts";

const Host = native.NativeTuiHost as
  | (new (width: number, height: number, headless: boolean) => NativeTuiHostContract)
  | undefined;

const canRun = Host !== undefined;

function tracked<P>(render: (props: P) => View): { component: ViewComponent<P>; calls: () => number } {
  let calls = 0;
  const component = defineView<P>((props) => {
    calls += 1;
    return render(props);
  });
  return { component, calls: () => calls };
}

function bridgeTextOf(view: View | undefined): string | undefined {
  if (view === undefined) return undefined;
  const node = nodeForBridge(view) as { kind: unknown; spans: readonly { text?: string }[] };
  if (node.kind !== BRIDGE_VIEW_KIND.text) throw new Error(`expected text node`);
  return node.spans[0]?.text;
}

describe("T13.1 R4 — tracked State<T>", () => {
  test("purity: writing inside a component body rejects deterministically", () => {
    const runtime = new RetainedExecutionRuntime();
    const counter = state(0);
    const Violent = defineView(() => {
      counter.set(99); // forbidden inside bodies
      return composeText("never");
    });
    expect(() => runtime.mountRoot(Violent, {})).toThrow(
      /cannot be written while a component body is evaluating/,
    );
    // The value was NOT mutated and nothing was committed.
    expect(counter.value).toBe(0);
    runtime.dispose();
  });

  test("subscription lifecycle: deps dropped when no longer read", () => {
    const runtime = new RetainedExecutionRuntime();
    let showExtra = false;
    const extraState = state("extra");
    const baseState = state("base");
    const S = tracked(() => {
      const parts = [baseState.value];
      if (showExtra) parts.push(extraState.value);
      return composeText(parts.join("+"));
    });
    const Holder = tracked(() => composeVertical((column) => column.child(S.component({}))));
    const root = runtime.mountRoot(Holder.component, {});

    // Subscribe to extra by reading it once.
    showExtra = true;
    runtime.update(root);
    const callsWithExtra = S.calls();

    // Stop reading it; commit the reduced dependency set.
    showExtra = false;
    runtime.update(root);
    const afterShrink = S.calls();

    // Writes to the dropped dependency must NOT reach the scope...
    extraState.set("changed");
    runtime.flush();
    expect(S.calls()).toBe(afterShrink);

    // ...while writes to the live dependency still do.
    baseState.set("base2");
    runtime.flush();
    expect(S.calls()).toBe(afterShrink + 1);
    void callsWithExtra;
    runtime.dispose();
  });

  test("abort retains the COMMITTED dependency set", () => {
    const runtime = new RetainedExecutionRuntime();
    let failAfterRead = false;
    let readSecond = false;
    const first = state("f1");
    const second = state("s1");
    const S = tracked(() => {
      const a = first.value;
      let b: string | undefined;
      if (readSecond) b = second.value;
      if (failAfterRead) throw new Error("boom-after-read");
      return composeText(a + (b ?? ""));
    });
    const Holder = tracked(() => composeVertical((column) => column.child(S.component({}))));
    const root = runtime.mountRoot(Holder.component, {});
    const callsAfterMount = S.calls();

    // Evaluate, read `second`, then fail: pending deps are discarded on abort.
    // Direct scope invalidation: the holder's props are unchanged, so the
    // skip gate would otherwise bypass S's body (correct behavior) — this
    // test deliberately routes around it.
    readSecond = true;
    failAfterRead = true;
    const sScope = root.children[0]!.scope;
    expect(() => runtime.update(sScope)).toThrow("boom-after-read");
    expect(S.calls()).toBe(callsAfterMount + 1);

    // The aborted read must NOT have subscribed us to `second`. Pinned at the
    // subscriber level: since the post-R9 review, an aborted pass RESTORES its
    // dirty obligation (§32.3), so a later flush re-runs this scope and a
    // behavioral "no execution" probe can no longer isolate subscription state.
    expect(trackedStateSubscriberCount(second)).toBe(0);
    expect(trackedStateSubscriberCount(first)).toBe(1); // committed set retained

    // The restored obligation re-executes while the bomb is armed — invalida-
    // tion ≠ success — and STILL does not subscribe the aborted read.
    expect(() => runtime.flush()).toThrow("boom-after-read");
    expect(trackedStateSubscriberCount(second)).toBe(0);
    expect(S.calls()).toBe(callsAfterMount + 2);

    // Clear the failure: the SAME preserved obligation now commits, adopting
    // BOTH reads going forward. No new invalidation was required.
    failAfterRead = false;
    runtime.flush();
    const adopted = S.calls();

    // Both dependencies now drive invalidation.
    first.set("f3");
    runtime.flush();
    expect(S.calls()).toBe(adopted + 1);
    second.set("s3");
    runtime.flush();
    expect(S.calls()).toBe(adopted + 2);
    runtime.dispose();
  });

  test("Object.is discipline: same-value sets are silent no-ops", () => {
    const runtime = new RetainedExecutionRuntime();
    const count = state(7);
    const obj = { id: 1 };
    const holder = state(obj);
    const T = tracked(() => composeText(`c${count.value}`));
    const Holder = tracked(() => composeVertical((column) => column.child(T.component({}))));
    const root = runtime.mountRoot(Holder.component, {});
    const before = executionCounterSnapshot();

    count.set(7); // same primitive -> silent
    holder.set(obj); // same reference -> silent
    runtime.flush();

    const after = executionCounterSnapshot();
    expect(after.execution_scope_state_invalidations - before.execution_scope_state_invalidations).toBe(0);
    expect(after.execution_scope_dirty_enqueues - before.execution_scope_dirty_enqueues).toBe(0);
    expect(count.value).toBe(7);

    // update() applies the transition function end-to-end.
    count.update((previous) => previous + 1);
    runtime.flush();
    expect(count.value).toBe(8);
    runtime.dispose();
  });

  test("shared state fans out to every subscriber in one pass", () => {
    const runtime = new RetainedExecutionRuntime();
    const shared = state("go");
    const A = tracked(() => composeText(`A${shared.value}`));
    const B = tracked(() => composeText(`B${shared.value}`));
    const Holder = tracked(() =>
      composeVertical((column) => {
        column.child(A.component({}));
        column.child(B.component({}));
      }),
    );
    const root = runtime.mountRoot(Holder.component, {});
    const base = { a: A.calls(), b: B.calls() };

    shared.set("stop");
    const before = executionCounterSnapshot();
    runtime.flush();
    const after = executionCounterSnapshot();

    expect(A.calls()).toBe(base.a + 1);
    expect(B.calls()).toBe(base.b + 1);
    expect(after.execution_flush_passes - before.execution_flush_passes).toBe(1);
    runtime.dispose();
  });

  test("disposed scopes are unsubscribed: writes after dispose are safe no-ops", () => {
    const runtime = new RetainedExecutionRuntime();
    const s = state("v");
    const T = tracked(() => composeText(s.value));
    runtime.mountRoot(T.component, {});
    runtime.dispose();
    expect(() => s.set("after")).not.toThrow();
    expect(s.value).toBe("after");
    expect(trackedStateSubscriberCount(s)).toBe(0);
  });

  describe.skipIf(!canRun)("with native projections", () => {
    function makeRuntime(): { runtime: RetainedExecutionRuntime; host: NativeTuiHostContract } {
      const host = new Host!(48, 12, true);
      const runtime = new RetainedExecutionRuntime({
        createScopeProjection: () => {
          const slot = new ViewSlot(host, View.spacer(0));
          const view = slot.view();
          return {
            view,
            install(output: View): void {
              slot.setView(output);
            },
            preparePublication(output: View) {
              return slot.prepareSetView(output);
            },
            dispose(): void {
              slot.dispose();
            },
          };
        },
      });
      return { runtime, host };
    }

    test("§31.1 execution-frontier gate END-TO-END: write B ⇒ App=0 A=0 B=1 C=0", () => {
      const { runtime, host } = makeRuntime();
      try {
        const stateA = state("a");
        const stateB = state("b");
        const stateC = state("c");

        const CompA = tracked(() => composeText(`A=${stateA.value}`));
        const CompB = tracked(() => composeText(`B=${stateB.value}`));
        const CompC = tracked(() => composeText(`C=${stateC.value}`));

        const App = tracked(() =>
          composeVertical((column) => {
            column.child(CompA.component({}));
            column.child(CompB.component({}));
            column.child(CompC.component({}));
          }),
        );
        const root = runtime.mountRoot(App.component, {});
        const [, scopeB] = root.children.map((record) => record.scope);

        const baselineCalls = { app: App.calls(), a: CompA.calls(), b: CompB.calls(), c: CompC.calls() };
        const before = executionCounterSnapshot();

        stateB.set("written"); // ONE local tracked-state write

        // The write alone schedules the work: no explicit invalidate call.
        runtime.flush();

        const after = executionCounterSnapshot();
        expect(App.calls()).toBe(baselineCalls.app); // parent body: 0
        expect(CompA.calls()).toBe(baselineCalls.a); // A body: 0
        expect(CompB.calls()).toBe(baselineCalls.b + 1); // B body: exactly 1
        expect(CompC.calls()).toBe(baselineCalls.c); // C body: 0
        expect(after.execution_scope_state_invalidations - before.execution_scope_state_invalidations).toBe(1);

        // Semantic frontier: only B's content changed.
        expect(bridgeTextOf(scopeB.currentOutput)).toBe("B=written");
        runtime.dispose();
      } finally {
        host.dispose();
      }
    });

    test("batching: two states read by one scope run its body once per flush", () => {
      const { runtime, host } = makeRuntime();
      try {
        const width = state(10);
        const height = state(20);
        const Box = tracked(() => composeText(`box ${width.value}x${height.value}`));
        const Holder = tracked(() => composeVertical((column) => column.child(Box.component({}))));
        const root = runtime.mountRoot(Holder.component, {});
        const callsAfterMount = Box.calls();
        const passesBefore = executionCounterSnapshot().execution_flush_passes;

        width.set(11);
        height.set(21);
        runtime.flush(); // one flush for two writes

        expect(Box.calls()).toBe(callsAfterMount + 1);
        expect(executionCounterSnapshot().execution_flush_passes - passesBefore).toBe(1);
        expect(bridgeTextOf(root.children[0]!.scope.currentOutput)).toBe("box 11x21");
        runtime.dispose();
      } finally {
        host.dispose();
      }
    });
  });
});
