/**
 * PERF-12 T13.1 R5 — dirty scheduler & batching proofs (handoff §32.1 R5,
 * AMENDMENT-C §12/§17/§18 Step 8R).
 *
 * Gate evidence:
 *   - 10 synchronous writes ⇒ exactly ONE flush pass and ONE commit batch;
 *   - auto-scheduling: first invalidation in a turn schedules one flush,
 *     later invalidations join it (§12.1); explicit flush pre-empts without
 *     double execution;
 *   - parent-before-child determinism: when parent AND child are dirty in the
 *     same batch and the parent supplies newer inputs, the child executes
 *     EXACTLY ONCE (no double execution, §12.2);
 *   - a removed-while-dirty child's queued work is discarded after the
 *     parent's structural commit;
 *   - duplicate invalidations coalesce;
 *   - scenario H smoke: several independent writes land as one transaction;
 *   - staged-failure injection leaves no partial committed state observable
 *     beyond the sanctioned pass boundary (§22.3).
 */

import { describe, expect, test } from "bun:test";
import {
  RetainedExecutionRuntime,
  executionCounterSnapshot,
} from "../src/tui/execution.ts";
import { defineView } from "../src/tui/define-view.ts";
import { invokeComponent } from "../src/tui/execution.ts";
import { state } from "../src/tui/tracked-state.ts";
import { composeText, composeVertical } from "../src/tui/compose.ts";

function tracked<P>(render: (props: P) => View): { component: ViewComponent<P>; calls: () => number } {
  let calls = 0;
  const component = defineView<P>((props) => {
    calls += 1;
    return render(props);
  });
  return { component, calls: () => calls };
}

describe("T13.1 R5 — dirty scheduler & batching", () => {
  test("10 synchronous writes ⇒ exactly one flush pass and one commit batch", () => {
    const runtime = new RetainedExecutionRuntime({ autoFlush: false });
    const scopes = Array.from({ length: 10 }, (_, index) => {
      const T = tracked(() => composeText(`t${index}`));
      const root = runtime.mountRoot(T.component, {});
      return { T, root };
    });
    const baseCalls = scopes.map((entry) => entry.T.calls());
    const before = executionCounterSnapshot();

    for (const entry of scopes) runtime.invalidate(entry.root);
    expect(executionCounterSnapshot().execution_flush_passes).toBe(before.execution_flush_passes);

    runtime.flush();

    const after = executionCounterSnapshot();
    expect(after.execution_flush_passes - before.execution_flush_passes).toBe(1);
    expect(after.execution_commit_batches - before.execution_commit_batches).toBe(1);
    scopes.forEach((entry, index) => {
      expect(entry.T.calls()).toBe(baseCalls[index]! + 1);
    });
    runtime.dispose();
  });

  test("auto-scheduling: burst of invalidations coalesces into one flush", async () => {
    const runtime = new RetainedExecutionRuntime(); // autoFlush default: true
    const T = tracked(() => composeText("auto"));
    const root = runtime.mountRoot(T.component, {});
    const baseCalls = T.calls();
    const passesBefore = executionCounterSnapshot().execution_flush_passes;

    // Burst: nothing runs synchronously...
    for (let index = 0; index < 10; index += 1) runtime.invalidate(root);
    expect(T.calls()).toBe(baseCalls);

    // ...one microtask turn later, exactly one flush has consumed them all.
    await Promise.resolve();
    expect(T.calls()).toBe(baseCalls + 1);
    expect(executionCounterSnapshot().execution_flush_passes - passesBefore).toBe(1);
    runtime.dispose();
  });

  test("explicit flush pre-empts the scheduled flush without double execution", async () => {
    const runtime = new RetainedExecutionRuntime();
    const T = tracked(() => composeText("preempt"));
    const root = runtime.mountRoot(T.component, {});
    const baseCalls = T.calls();

    runtime.invalidate(root); // schedules a microtask flush
    runtime.flush(); // explicit, immediate

    await Promise.resolve(); // the scheduled callback finds an empty queue

    expect(T.calls()).toBe(baseCalls + 1);
    runtime.dispose();
  });

  test("§12.2: parent and child both dirty ⇒ child executes exactly once", async () => {
    const runtime = new RetainedExecutionRuntime();
    const ownState = state("s0");
    let propValue = "v0";
    const Child = tracked(({ v }: { v: string }) => composeText(`${v}:${ownState.value}`));
    const Sibling = tracked(() => composeText("sibling"));
    const Parent = tracked(() =>
      composeVertical((column) => {
        column.child(Sibling.component({}));
        column.child(Child.component({ v: propValue }));
      }),
    );
    const root = runtime.mountRoot(Parent.component, {});
    const childScope = root.children[1]!.scope;
    const baseCalls = { parent: Parent.calls(), child: Child.calls(), sibling: Sibling.calls() };

    // BOTH dirty in one turn: the child via its own tracked state, the parent
    // via an external invalidation that also carries newer child props.
    ownState.set("s1");
    propValue = "v1";
    runtime.invalidate(childScope);
    runtime.update(root);

    expect(Child.calls()).toBe(baseCalls.child + 1); // superseded, not doubled
    expect(Sibling.calls()).toBe(baseCalls.sibling);

    // Final semantics reflect the newest inputs.
    const textNode = bridgeTextOfNullable(root.children[1]!.scope.currentOutput);
    expect(textNode).toBe("v1:s1");
    runtime.dispose();
  });

  test("removed-while-dirty child: queued work discarded after structural commit", () => {
    const runtime = new RetainedExecutionRuntime({ autoFlush: false });
    let includeB = true;
    const stateB = state("live");
    const A = tracked(() => composeText("a"));
    const B = tracked(() => composeText(`b:${stateB.value}`));
    const C = tracked(() => composeText("c"));
    const Parent = tracked(() =>
      composeVertical((column) => {
        column.child(A.component({}));
        column.child(C.component({}));
        // Trailing conditional (handoff §8.3/§16 pattern): its removal cannot
        // shift earlier siblings' ordinals.
        if (includeB) column.child(B.component({}));
      }),
    );
    const root = runtime.mountRoot(Parent.component, {});
    const scopeB = root.children[2]!.scope;
    const baseCalls = { b: B.calls(), c: C.calls() };

    // B becomes dirty, THEN the parent structurally removes it in the same turn.
    includeB = false;
    runtime.invalidate(scopeB);
    runtime.invalidate(root);
    runtime.flush();

    expect(B.calls()).toBe(baseCalls.b); // discarded before execution
    expect(scopeB.disposed).toBe(true); // unmounted by the structural commit
    expect(C.calls()).toBe(baseCalls.c); // stable sibling untouched
    runtime.dispose();
  });

  test("middle-child removal under positional identity: later siblings legitimately remount (pre-R8-keys semantics)", () => {
    const runtime = new RetainedExecutionRuntime({ autoFlush: false });
    let includeMiddle = true;
    const Head = tracked(() => composeText("head"));
    const Middle = tracked(() => composeText("m"));
    const Tail = tracked(() => composeText("tail"));
    const Parent = tracked(() =>
      composeVertical((column) => {
        column.child(Head.component({}));
        if (includeMiddle) column.child(Middle.component({}));
        column.child(Tail.component({}));
      }),
    );
    const root = runtime.mountRoot(Parent.component, {});
    const tailScopeBefore = root.children[2]?.scope;

    // Removing the MIDDLE child shifts the tail's ordinal: positional
    // reconciliation treats it as a replacement (fresh scope, body re-runs)
    // — documented pre-R8-keys behavior; keys make this continuous.
    includeMiddle = false;
    const before = executionCounterSnapshot();
    runtime.update(root);
    const after = executionCounterSnapshot();

    expect(root.children.length).toBe(2);
    expect(after.execution_scope_mounts - before.execution_scope_mounts).toBe(1); // fresh tail scope
    expect(Tail.calls()).toBe(2); // initial mount + positional remount
    void tailScopeBefore;
    runtime.dispose();
  });

  test("scenario H smoke: several independent State writes land as one transaction", () => {
    const runtime = new RetainedExecutionRuntime();
    const sA = state("a1");
    const sB = state("b1");
    const sC = state("c1");
    const A = tracked(() => composeText(sA.value));
    const B = tracked(() => composeText(sB.value));
    const C = tracked(() => composeText(sC.value));
    const Holder = tracked(() =>
      composeVertical((column) => {
        column.child(A.component({}));
        column.child(B.component({}));
        column.child(C.component({}));
      }),
    );
    const root = runtime.mountRoot(Holder.component, {});
    const base = { a: A.calls(), b: B.calls(), c: C.calls() };
    const before = executionCounterSnapshot();

    sA.set("a2");
    sB.set("b2");
    sC.set("c2");
    runtime.flush();

    const after = executionCounterSnapshot();
    expect(A.calls()).toBe(base.a + 1);
    expect(B.calls()).toBe(base.b + 1);
    expect(C.calls()).toBe(base.c + 1);
    expect(after.execution_flush_passes - before.execution_flush_passes).toBe(1);
    expect(after.execution_commit_batches - before.execution_commit_batches).toBe(1);
    runtime.dispose();
  });

  test("staged failure mid-batch: committed state stays authoritative; retry succeeds", () => {
    const runtime = new RetainedExecutionRuntime({ autoFlush: false });
    const stableState = state("stable");
    let boom = false;
    const Stable = tracked(() => composeText(stableState.value));
    const Bomber = tracked(() => {
      if (boom) throw new Error("boom");
      return composeText("fine");
    });
    const Holder = tracked(() =>
      composeVertical((column) => {
        column.child(Stable.component({}));
        column.child(Bomber.component({}));
      }),
    );
    const root = runtime.mountRoot(Holder.component, {});
    const stableScope = root.children[0]!.scope;
    const bomberScope = root.children[1]!.scope;
    const stableBefore = bridgeTextOfNullable(stableScope.currentOutput);

    stableState.set("changed");
    boom = true;
    runtime.invalidate(stableScope);
    runtime.invalidate(bomberScope);
    expect(() => runtime.flush()).toThrow("boom");

    // Nothing partial leaked: the stable scope still presents the OLD value.
    expect(bridgeTextOfNullable(stableScope.currentOutput)).toBe(stableBefore);

    boom = false;
    runtime.update(stableScope);
    expect(bridgeTextOfNullable(stableScope.currentOutput)).toBe("changed");
    runtime.dispose();
  });

  test("duplicate invalidations coalesce (re-pin)", () => {
    const runtime = new RetainedExecutionRuntime({ autoFlush: false });
    const T = tracked(() => composeText("dup"));
    const root = runtime.mountRoot(T.component, {});
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
});

function bridgeTextOfNullable(view: import("../src/tui/values/view.ts").View | undefined): string | undefined {
  if (view === undefined) return undefined;
  const node = nodeForBridge(view) as { kind: unknown; spans: readonly { text?: string }[] };
  if (node.kind !== BRIDGE_VIEW_KIND.text) throw new Error(`expected text node`);
  return node.spans[0]?.text;
}

import { BRIDGE_VIEW_KIND } from "../src/tui/ir.ts";
import { View, nodeForBridge } from "../src/tui/values/view.ts";
import type { ViewComponent } from "../src/tui/execution.ts";
