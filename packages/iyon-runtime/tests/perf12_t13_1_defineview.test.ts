/**
 * PERF-12 T13.1 R2 — public `defineView` component API proofs (handoff §32.1
 * R2, AMENDMENT-C §6/§18 Step 5R).
 *
 * Gate evidence:
 *   - invocation returns a stable View for the parent to embed;
 *   - props skip proven by body-call counters: unchanged primitive props
 *     (even in FRESH object literals) ⇒ 0 executions;
 *   - fresh literals carrying identity-valued fields (nested objects) are
 *     correctly NOT skipped — the Review Addendum §33.6 contract, documented
 *     by test;
 *   - positional identity: same ordinal ⇒ same instance, distinct ordinals
 *     ⇒ distinct instances of the SAME component;
 *   - local-key plumbing records keys on child scopes (keyed reconciliation
 *     dynamics arrive in R8);
 *   - deterministic errors outside evaluation / for non-components.
 */

import { describe, expect, test } from "bun:test";
import {
  RetainedExecutionRuntime,
  executionCounterSnapshot,
  type ViewComponent,
} from "../src/tui/execution.ts";
import { defineView } from "../src/tui/define-view.ts";
import { invokeComponent, keyGroupOf } from "../src/tui/execution.ts";
import { composeText, composeVertical } from "../src/tui/compose.ts";
import { BRIDGE_VIEW_KIND } from "../src/tui/ir.ts";
import { View, nodeForBridge } from "../src/tui/values/view.ts";

/** Test component factory with body-call accounting. */
function tracked<P>(render: (props: P) => View): { component: ViewComponent<P>; calls: () => number } {
  let calls = 0;
  const component = defineView<P>((props) => {
    calls += 1;
    return render(props);
  });
  return { component, calls: () => calls };
}

describe("T13.1 R2 — defineView public API", () => {
  test("defineView returns a callable component carrying its render entry", () => {
    const Footer = defineView<{ status: string }>(({ status }) => View.text(status));
    expect(typeof Footer).toBe("function");
    expect(typeof Footer.render).toBe("function");
    expect(() => defineView(42 as never)).toThrow(TypeError);
  });

  test("invocation inside a parent scope embeds a semantically correct View", () => {
    const runtime = new RetainedExecutionRuntime();
    const Footer = tracked(({ status }: { status: string }) =>
      composeText(`footer ${status}`),
    );
    const App = tracked(() =>
      composeVertical((column) => column.child(Footer.component({ status: "ready" }))),
    );
    const root = runtime.mountRoot(App.component, {});

    // The embedded text must equal direct construction semantics.
    const stripIds = (_key: string, v: unknown): unknown => (_key === "id" ? undefined : v);
    const reference = View.vertical((column) => column.child(View.text("footer ready")));
    expect(JSON.stringify(nodeForBridge(root.currentOutput!), stripIds))
      .toBe(JSON.stringify(nodeForBridge(reference), stripIds));
    runtime.dispose();
  });

  test("positional identity: same ordinal ⇒ same instance; distinct ordinals ⇒ distinct instances", () => {
    const runtime = new RetainedExecutionRuntime();
    const Cell = tracked(({ label }: { label: string }) => composeText(label));
    const seenA: unknown[] = [];
    const seenB: unknown[] = [];
    let labelB = "b1";
    const Row = tracked(() =>
      composeVertical((row) => {
        seenA.push(invokeComponent(Cell.component, { label: "a" }).scope);
        seenB.push(invokeComponent(Cell.component, { label: labelB }).scope);
        row.child(composeText("spacer"));
      }),
    );
    const root = runtime.mountRoot(Row.component, {});
    const firstA = seenA[0];
    const firstB = seenB[0];
    expect(firstA).not.toBe(firstB); // same component, different ordinals

    labelB = "b2";
    runtime.update(root);
    expect(seenA[1]).toBe(firstA); // surviving instance at ordinal 0
    expect(seenB[1]).toBe(firstB); // surviving instance at ordinal 1
    runtime.dispose();
  });

  test("props skip: unchanged primitive props in a FRESH literal skip the body", () => {
    const runtime = new RetainedExecutionRuntime();
    const Child = tracked(({ v }: { v: number }) => composeText(`child ${v}`));
    let value = 1;
    const Parent = tracked(() =>
      composeVertical((column) => column.child(Child.component({ v: value }))),
    );
    const root = runtime.mountRoot(Parent.component, {});
    const callsAfterMount = Child.calls();

    // Fresh literal, equal primitive field -> skipped (Object.is per key).
    value = 1;
    runtime.update(root);
    expect(Child.calls()).toBe(callsAfterMount);

    // Changed primitive field -> exactly one more execution.
    value = 2;
    runtime.update(root);
    expect(Child.calls()).toBe(callsAfterMount + 1);
    runtime.dispose();
  });

  test("props contract: fresh literal with an identity-valued field is NOT skipped (§33.6)", () => {
    const runtime = new RetainedExecutionRuntime();
    const Child = tracked(({ style }: { style: { tone: string } }) => composeText(style.tone));
    let passStable = false;
    const stableStyle = { tone: "muted" };
    // Before `passStable`: a FRESH nested literal every evaluation.
    // After: the SAME stable object reference every evaluation.
    const Parent = tracked(() =>
      composeVertical((column) =>
        column.child(
          Child.component({ style: passStable ? stableStyle : { tone: "muted" } }),
        ),
      ),
    );
    const root = runtime.mountRoot(Parent.component, {});
    const callsAfterMount = Child.calls();

    // Fresh nested object every render -> identity differs -> NO skip.
    runtime.update(root);
    expect(Child.calls()).toBe(callsAfterMount + 1);

    // Switch to the stable reference: first execution reconciles identity
    // (one more call), then identical references skip again.
    passStable = true;
    runtime.update(root);
    expect(Child.calls()).toBe(callsAfterMount + 2);

    runtime.update(root);
    expect(Child.calls()).toBe(callsAfterMount + 2);
    runtime.dispose();
  });

  test("local-key plumbing records the key on the child scope (reconciliation dynamics: R8)", () => {
    const runtime = new RetainedExecutionRuntime();
    const Item = tracked(({ label }: { label: string }) => composeText(label));
    const keys: unknown[] = [];
    const List = tracked(() =>
      composeVertical((column) => {
        const { scope } = invokeComponent(Item.component, { label: `tool-7` }, `tool-7`);
        keys.push(keyGroupOf(scope));
        column.child(composeText("tail"));
      }),
    );
    runtime.mountRoot(List.component, {});
    expect(keys[0]).toBe("tool-7");
    runtime.dispose();
  });

  test("component-type check: non-component values are rejected deterministically", () => {
    const runtime = new RetainedExecutionRuntime();
    const NotAComponent = { noRenderHere: true };
    const Bad = tracked(() =>
      composeVertical((column) => {
        column.child(invokeComponent(NotAComponent as never, {}).view);
      }),
    );
    try {
      runtime.mountRoot(Bad.component, {});
      throw new Error("expected mountRoot to reject non-components");
    } catch (error) {
      expect((error as { code?: string }).code).toBe("TUI_EXECUTION_NOT_A_COMPONENT");
    }
    runtime.dispose();
  });

  test("invocation outside any evaluating scope is a deterministic error", () => {
    const Footer = defineView<{ status: string }>(({ status }) => View.text(status));
    expect(() => Footer({ status: "x" })).toThrow(/outside any evaluating scope/);
  });

  test("async render bodies are rejected through the public path", () => {
    const runtime = new RetainedExecutionRuntime();
    const Async = defineView(() => new Promise<View>(() => {}) as unknown as View);
    const Holder = tracked(() => composeVertical((column) => column.child(Async({}))));
    try {
      runtime.mountRoot(Holder.component, {});
      throw new Error("expected mountRoot to reject async renders");
    } catch (error) {
      expect((error as { code?: string }).code).toBe("TUI_EXECUTION_ASYNC_BODY");
    }
    runtime.dispose();
  });

  test("production-chrome smoke: exact repeat skips all bodies; footer change executes one", () => {
    const runtime = new RetainedExecutionRuntime();
    const Working = tracked(() => composeText("working…"));
    const Approval = tracked(() => composeText("approve?"));
    const Composer = tracked(() => composeText("|"));
    const Footer = tracked(({ status }: { status: string }) => composeText(`status ${status}`));

    interface ChromeProps {
      readonly workingVisible: boolean;
      readonly status: string;
    }
    const App = defineView<ChromeProps>(({ workingVisible, status }) =>
      composeVertical((column) => {
        // Stable children FIRST (fixed ordinals); the conditional element is
        // TRAILING so its toggle cannot shift sibling ordinals (§8.3 pattern;
        // leading conditionals + positional identity legitimately remount
        // later siblings until keyed reconciliation lands in R8).
        column.child(Approval.component({}));
        column.child(Composer.component({}));
        column.child(Footer.component({ status }));
        if (workingVisible) column.child(Working.component({}));
      }),
    );

    // NOTE: root-level props UPDATE channels arrive with canonical boundary
    // wiring (R8/R11); at this tranche the props object is handed to mount
    // and top-level field mutations are observed on parent re-execution.
    const chromeProps: ChromeProps = { workingVisible: true, status: "ready" };
    const root = runtime.mountRoot(App, chromeProps);
    const callsBaseline = {
      working: Working.calls(),
      approval: Approval.calls(),
      composer: Composer.calls(),
      footer: Footer.calls(),
    };

    // Exact repeat: same props -> all child bodies skip.
    const beforeRepeat = executionCounterSnapshot();
    runtime.update(root);
    expect(Working.calls()).toBe(callsBaseline.working);
    expect(Approval.calls()).toBe(callsBaseline.approval);
    expect(Composer.calls()).toBe(callsBaseline.composer);
    expect(Footer.calls()).toBe(callsBaseline.footer);
    const afterRepeat = executionCounterSnapshot();
    expect(afterRepeat.execution_scope_noop_outputs - beforeRepeat.execution_scope_noop_outputs).toBeGreaterThanOrEqual(1);

    // Footer-only change: per-key Object.is sees the mutated scalar; exactly
    // one body executes.
    (chromeProps as { status: string }).status = "running tool 3/7";
    const beforeFooter = executionCounterSnapshot();
    runtime.invalidate(root);
    runtime.flush();
    expect(Footer.calls()).toBe(callsBaseline.footer + 1);
    expect(Working.calls()).toBe(callsBaseline.working);
    expect(Approval.calls()).toBe(callsBaseline.approval);
    expect(Composer.calls()).toBe(callsBaseline.composer);
    const afterFooter = executionCounterSnapshot();
    expect(afterFooter.execution_scope_changed_outputs - beforeFooter.execution_scope_changed_outputs).toBeGreaterThanOrEqual(1);

    // Structural change owned by the parent: unchanged children keep skipping;
    // the trailing conditional mounts/unmounts without disturbing siblings.
    (chromeProps as { workingVisible: boolean }).workingVisible = false;
    runtime.update(root);
    expect(Footer.calls()).toBe(callsBaseline.footer + 1);
    expect(Approval.calls()).toBe(callsBaseline.approval);
    expect(Composer.calls()).toBe(callsBaseline.composer);

    // And back: re-mounts fresh at the trailing ordinal.
    (chromeProps as { workingVisible: boolean }).workingVisible = true;
    runtime.update(root);
    expect(Working.calls()).toBe(callsBaseline.working + 1); // initial mount + one remount
    runtime.dispose();
  });
});
