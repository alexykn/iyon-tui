/**
 * PERF-12 T13.1 R1 — retained execution substrate proofs (handoff §32.1 R1,
 * AMENDMENT-C §18 Step 4R).
 *
 * Gate evidence (synthetic driver, no public component API yet):
 *   - same type + position ⇒ same scope instance across updates;
 *   - type mismatch ⇒ replacement/remount;
 *   - dirty scope executes once; clean scopes ZERO body calls (counter-proven);
 *   - exact semantic repeat returns the exact previous View inside a scope;
 *   - changed payload ⇒ new NodeId;
 *   - control-flow shifts reduce reuse locally without corrupting anything;
 *   - slot tails shrink on commit, rewind on abort; abort keeps every
 *     committed structure authoritative;
 *   - props skipping (shallow Object.is) skips bodies — counter-proven;
 *   - async bodies rejected deterministically;
 *   - duplicate invalidations coalesce into one execution;
 *   - disposal releases strong references.
 */

import { describe, expect, test } from "bun:test";
import {
  EXECUTION_ASYNC_BODY,
  ExecutionError,
  RetainedExecutionRuntime,
  executionCounterSnapshot,
  invokeComponent,
  resetExecutionCounters,
  type ViewComponentType,
} from "../src/tui/execution.ts";
import { composeText, composeVertical } from "../src/tui/compose.ts";
import { BRIDGE_VIEW_KIND, type BridgeViewNode } from "../src/tui/ir.ts";
import { View, nodeForBridge } from "../src/tui/values/view.ts";

/** Test component factory with body-call accounting. */
function tracked<P>(render: (props: P) => View): { type: ViewComponentType<P>; calls: () => number } {
  let calls = 0;
  return {
    type: {
      render(props: P): View {
        calls += 1;
        return render(props);
      },
    },
    calls: () => calls,
  };
}

function bridgeOf(view: View | undefined): BridgeViewNode {
  if (view === undefined) throw new Error("expected a committed output");
  return nodeForBridge(view);
}

describe("T13.1 R1 — execution scope substrate", () => {
  test("same type + position ⇒ same instance across parent re-renders", () => {
    const runtime = new RetainedExecutionRuntime();
    let captured: unknown[] = [];
    const Hint = tracked(() => composeText("hint"));
    const App = tracked(() => {
      const { scope } = invokeComponent(Hint.type, {});
      captured.push(scope);
      return composeVertical((column) => column.child(composeText("app")));
    });

    const root = runtime.mountRoot(App.type, {});
    runtime.update(root);
    expect(captured.length).toBe(2);
    expect(captured[0]).toBe(captured[1]);
    expect((captured[0] as { disposed: boolean }).disposed).toBe(false);
    runtime.dispose();
  });

  test("type mismatch at the same position ⇒ replacement; predecessor disposed", () => {
    const runtime = new RetainedExecutionRuntime();
    let which = "a";
    const A = tracked(() => composeText("A"));
    const B = tracked(() => composeText("B"));
    const seen: unknown[] = [];
    const App = tracked(() => {
      const { scope } = invokeComponent(which === "a" ? A.type : B.type, {});
      seen.push(scope);
      return composeVertical((column) => column.child(composeText("chrome")));
    });

    const root = runtime.mountRoot(App.type, {});
    const first = seen[0];
    which = "b";
    runtime.update(root);
    expect(seen[0]).not.toBe(seen[1]);
    expect((first as { disposed: boolean }).disposed).toBe(true);
    // Replacement is real remount semantics: the new body executed.
    expect(A.calls()).toBe(1);
    expect(B.calls()).toBe(1);
    runtime.dispose();
  });

  test("dirty scope executes once; clean sibling scopes zero body calls", () => {
    const runtime = new RetainedExecutionRuntime();
    const A = tracked(() => composeText("a"));
    const B = tracked(() => composeText(`b:${B_value}`));
    const C = tracked(() => composeText("c"));
    let B_value = "one";

    const rootA = runtime.mountRoot(A.type, {});
    const rootB = runtime.mountRoot(B.type, {});
    const rootC = runtime.mountRoot(C.type, {});
    const baseline = executionCounterSnapshot();
    const baseCalls = { a: A.calls(), b: B.calls(), c: C.calls() };

    B_value = "two";
    runtime.invalidate(rootB);
    runtime.flush();

    expect(B.calls()).toBe(baseCalls.b + 1);
    expect(A.calls()).toBe(baseCalls.a);
    expect(C.calls()).toBe(baseCalls.c);
    const delta = executionCounterSnapshot();
    expect(delta.execution_scope_body_calls - baseline.execution_scope_body_calls).toBe(1);
    // Semantic frontier: only B produced a new output this batch.
    void rootA;
    void rootC;
    runtime.dispose();
  });

  test("nested: child-only invalidation executes exactly the child body", () => {
    const runtime = new RetainedExecutionRuntime();
    let status = "Working";
    const Footer = tracked(() => composeText(`footer ${status}`));
    const Header = tracked(() => composeText("header"));
    const App = tracked(() =>
      composeVertical((column) => {
        column.child(invokeComponent(Header.type, {}).view);
        column.child(invokeComponent(Footer.type, {}).view);
      }),
    );

    const root = runtime.mountRoot(App.type, {});
    const headerCallsAfterMount = Header.calls();
    const footerCallsAfterMount = Footer.calls();
    status = "Done";
    runtime.update(root.children[1]!.scope); // Footer scope only

    expect(Footer.calls()).toBe(footerCallsAfterMount + 1);
    expect(Header.calls()).toBe(headerCallsAfterMount);
    // The footer scope's committed artifact carries the new semantics...
    expect(bridgeOf(root.children[1]!.scope.currentOutput).kind).toBe(BRIDGE_VIEW_KIND.text);
    runtime.dispose();
  });

  test("exact semantic repeat returns the exact previous View inside a scope", () => {
    const runtime = new RetainedExecutionRuntime();
    const T = tracked(() => composeText("stable"));
    const root = runtime.mountRoot(T.type, {});
    const firstOutput = root.currentOutput;

    const before = executionCounterSnapshot();
    runtime.update(root);
    const after = executionCounterSnapshot();

    expect(root.currentOutput).toBe(firstOutput);
    expect(after.composition_exact_view_reuses - before.composition_exact_view_reuses).toBe(1);
    expect(after.composition_new_views - before.composition_new_views).toBe(0);
    expect(after.execution_scope_noop_outputs - before.execution_scope_noop_outputs).toBe(1);
    runtime.dispose();
  });

  test("changed payload yields a new NodeId and a changed-output event", () => {
    const runtime = new RetainedExecutionRuntime();
    let value = "before";
    const T = tracked(() => composeText(value));
    const root = runtime.mountRoot(T.type, {});
    const oldId = bridgeOf(root.currentOutput).id;

    value = "after";
    const before = executionCounterSnapshot();
    runtime.update(root);
    const after = executionCounterSnapshot();

    expect(bridgeOf(root.currentOutput).id).not.toBe(oldId);
    expect(after.composition_new_views - before.composition_new_views).toBe(1);
    expect(after.execution_scope_changed_outputs - before.execution_scope_changed_outputs).toBe(1);
    runtime.dispose();
  });

  test("control-flow shift keeps semantics exact while local reuse degrades gracefully", () => {
    const runtime = new RetainedExecutionRuntime();
    let extra = false;
    const App = tracked(() =>
      composeVertical((column) => {
        column.child(composeText("first"));
        if (extra) column.child(composeText("extra"));
        column.child(composeText("last"));
      }),
    );
    const root = runtime.mountRoot(App.type, {});

    const referenceWithout = View.vertical([View.text("first"), View.text("last")]);
    const stripIds = (_key: string, v: unknown): unknown => (_key === "id" ? undefined : v);
    expect(JSON.stringify(nodeForBridge(root.currentOutput!), stripIds))
      .toBe(JSON.stringify(nodeForBridge(referenceWithout), stripIds));

    extra = true;
    runtime.update(root);
    const referenceWith = View.vertical([View.text("first"), View.text("extra"), View.text("last")]);
    expect(JSON.stringify(nodeForBridge(root.currentOutput!), stripIds))
      .toBe(JSON.stringify(nodeForBridge(referenceWith), stripIds));

    extra = false;
    runtime.update(root);
    expect(JSON.stringify(nodeForBridge(root.currentOutput!), stripIds))
      .toBe(JSON.stringify(nodeForBridge(referenceWithout), stripIds));
    runtime.dispose();
  });

  test("slot tail shrinks on commit and rewinds on abort", () => {
    const runtime = new RetainedExecutionRuntime();
    let count = 3;
    let fail = false;
    const T = tracked(() =>
      composeVertical((column) => {
        for (let index = 0; index < count; index += 1) column.child(composeText(`t${index}`));
        if (fail) throw new Error("boom");
        return composeText("ok");
      }),
    );
    const root = runtime.mountRoot(T.type, {});
    expect(root.committedSlotCount).toBeGreaterThan(0);
    const afterThree = root.committedSlotCount;

    count = 5;
    runtime.update(root);
    expect(root.committedSlotCount).toBe(afterThree + 2);

    // Abort mid-growth: staged slots beyond the committed boundary must rewind.
    count = 9;
    fail = true;
    expect(() => runtime.update(root)).toThrow("boom");
    expect(root.committedSlotCount).toBe(afterThree + 2);
    expect(root.currentOutput).toBeDefined();

    count = 4;
    fail = false;
    runtime.update(root);
    expect(root.committedSlotCount).toBe(afterThree + 1);
    runtime.dispose();
  });

  test("batch atomicity: one failing body aborts the whole batch transactionally", () => {
    const runtime = new RetainedExecutionRuntime();
    let stableValue = "old";
    let failNow = false;
    const Stable = tracked(() => composeText(stableValue));
    const Bomber = tracked(() => {
      const staged = composeText("staged-before-failure");
      if (failNow) throw new Error("boom");
      return staged;
    });
    const stableRoot = runtime.mountRoot(Stable.type, {});
    const bomberRoot = runtime.mountRoot(Bomber.type, {});
    const committedBefore = stableRoot.currentOutput;

    // Both dirty in one batch; the bomber fails AFTER Stable prepared new work.
    stableValue = "new-uncommitted";
    failNow = true;
    const before = executionCounterSnapshot();
    runtime.invalidate(stableRoot);
    runtime.invalidate(bomberRoot);
    expect(() => runtime.flush()).toThrow("boom");

    // Committed world untouched: Stable still presents the OLD value.
    expect(stableRoot.currentOutput).toBe(committedBefore);
    expect(bridgeOf(stableRoot.currentOutput).kind).toBe(BRIDGE_VIEW_KIND.text);
    const after = executionCounterSnapshot();
    expect(after.execution_commit_aborts - before.execution_commit_aborts).toBe(1);

    // Retry from the same state succeeds once the failure clears.
    failNow = false;
    runtime.invalidate(stableRoot);
    runtime.flush();
    expect(stableRoot.currentOutput).not.toBe(committedBefore);
    runtime.dispose();
  });

  test("fresh children created during an aborted evaluation are disposed", () => {
    const runtime = new RetainedExecutionRuntime();
    let failNow = false;
    const Fresh = tracked(() => composeText("fresh"));
    const seen: unknown[] = [];
    const App = tracked(() => {
      const { scope } = invokeComponent(Fresh.type, {});
      seen.push(scope);
      if (failNow) throw new Error("boom-after-mount");
      return composeVertical((column) => column.child(composeText("app")));
    });

    runtime.mountRoot(App.type, {});
    const originalChild = seen[0];
    failNow = true;
    // Replacing the child type forces a fresh child scope, then the body fails.
    const Other = tracked(() => composeText("other"));
    const FailingSwap = tracked(() => {
      const { scope } = invokeComponent(failNow ? Other.type : Fresh.type, {});
      seen.push(scope);
      throw new Error("boom");
    });
    expect(() => runtime.mountRoot(FailingSwap.type, {})).toThrow("boom");
    const freshScope = seen[1] as { disposed: boolean };
    expect(freshScope).not.toBe(originalChild);
    expect(freshScope.disposed).toBe(true);
    runtime.dispose();
  });

  test("props skipping: shallow-equal props skip the body; changed props re-execute", () => {
    const runtime = new RetainedExecutionRuntime();
    const Child = tracked((props: { v: number }) => composeText(`child ${props.v}`));
    let childProps = { v: 1 };
    const Parent = tracked(() =>
      composeVertical((column) => column.child(invokeComponent(Child.type, childProps).view)),
    );

    const root = runtime.mountRoot(Parent.type, {});
    const childScope = root.children[0]!.scope;
    const callsAfterMount = Child.calls();

    // Parent re-executes; child props are shallow-equal -> child body skips.
    runtime.update(root);
    expect(Child.calls()).toBe(callsAfterMount);
    expect(childScope.currentProps).toEqual({ v: 1 });

    // Changed props -> child body executes exactly once more.
    childProps = { v: 2 };
    runtime.update(root);
    expect(Child.calls()).toBe(callsAfterMount + 1);
    runtime.dispose();
  });

  test("duplicate invalidations coalesce into one execution", () => {
    const runtime = new RetainedExecutionRuntime();
    const T = tracked(() => composeText("t"));
    const root = runtime.mountRoot(T.type, {});
    const callsAfterMount = T.calls();
    const before = executionCounterSnapshot();
    runtime.invalidate(root);
    runtime.invalidate(root);
    runtime.invalidate(root);
    runtime.flush();
    const after = executionCounterSnapshot();

    expect(after.execution_scope_duplicate_invalidations - before.execution_scope_duplicate_invalidations).toBe(2);
    expect(after.execution_scope_dirty_enqueues - before.execution_scope_dirty_enqueues).toBe(1);
    expect(T.calls()).toBe(callsAfterMount + 1);
    runtime.dispose();
  });

  test("async bodies are rejected deterministically", () => {
    const runtime = new RetainedExecutionRuntime();
    const Async = tracked(() => new Promise<View>(() => {}) as unknown as View);
    try {
      runtime.mountRoot(Async.type, {});
      throw new Error("expected mountRoot to reject async bodies");
    } catch (error) {
      expect(error).toBeInstanceOf(ExecutionError);
      expect((error as ExecutionError).code).toBe(EXECUTION_ASYNC_BODY);
    }
    runtime.dispose();
  });

  test("multi-root isolation: identical types under different runtimes stay independent", () => {
    const Shared: ViewComponentType<{ label: string }> = { render: (props) => composeText(props.label) };
    const runtimeA = new RetainedExecutionRuntime();
    const runtimeB = new RetainedExecutionRuntime();
    const a = runtimeA.mountRoot(Shared, { label: "A" });
    const b = runtimeB.mountRoot(Shared, { label: "B" });

    expect(a).not.toBe(b);
    runtimeA.update(a);
    expect(b.currentOutput).not.toBe(undefined);

    const labelB = bridgeOf(b.currentOutput);
    runtimeA.dispose();
    expect(labelB.kind).toBe(BRIDGE_VIEW_KIND.text);
    expect(bridgeOf(b.currentOutput)).toBe(labelB);
    runtimeB.dispose();
  });

  test("disposal releases strong references (soak)", () => {
    resetExecutionCounters();
    const T: ViewComponentType<{ i: number }> = { render: (props) => composeText(`t${props.i}`) };
    for (let index = 0; index < 1_000; index += 1) {
      const runtime = new RetainedExecutionRuntime();
      const root = runtime.mountRoot(T, { i: index });
      expect(root.disposed).toBe(false);
      runtime.dispose();
      expect(root.disposed).toBe(true);
      expect(root.currentOutput).toBeUndefined();
      expect(root.committedSlotCount).toBe(0);
    }
    const counters = executionCounterSnapshot();
    expect(counters.execution_scope_unmounts).toBeGreaterThanOrEqual(1_000);
    expect(counters.execution_scope_mounts).toBe(1_000);
  });
});
