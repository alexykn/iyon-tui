/**
 * PERF-12 T13.1 R3 — retained scope projections (handoff §32.1 R3,
 * AMENDMENT-C §5/§14/§18 Step 6R).
 *
 * Every live execution scope owns an independently retained sub-DAG root
 * behind a STABLE component/ref view: the parent embeds the projection once,
 * and child content swaps happen behind it through the existing ViewSlot /
 * RetainedRootBoundary machinery (old root leased until replacement).
 *
 * Gate evidence:
 *   - the embedded projection keeps EXACT identity across child updates;
 *   - §31.3 semantic-DAG gate at 3-scope scale: a local B update leaves A/C
 *     outputs and every ScopeRef NodeId unchanged while B's content root
 *     advances;
 *   - semantic-noop invalidation performs ZERO installs (scenario I);
 *   - failed installs keep old content authoritative on both sides;
 *   - detached mode (no factory) preserves R1 raw-output embedding;
 *   - parent bodies stay unexecuted through the projection path.
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
import { composeText, composeVertical } from "../src/tui/compose.ts";
import { BRIDGE_VIEW_KIND } from "../src/tui/ir.ts";
import { View, nodeForBridge } from "../src/tui/values/view.ts";
import { ViewSlot } from "../src/tui/component.ts";

const Host = native.NativeTuiHost as
  | (new (width: number, height: number, headless: boolean) => NativeTuiHostContract)
  | undefined;

const canRun = Host !== undefined;

interface ProjectionProbe {
  readonly installs: () => number;
}

/** Projection factory backed by the existing public ViewSlot primitive, plus an install counter for assertions. */
function viewSlotProjectionFactory(
  host: NativeTuiHostContract,
  probes?: ProjectionProbe[],
) {
  return () => {
    const slot = new ViewSlot(host);
    const view = slot.view();
    let count = 0;
    const probe: ProjectionProbe = {
      installs: () => count,
    };
    probes?.push(probe);
    return {
      view,
      install(output: View): void {
        count += 1;
        slot.setView(output);
      },
      dispose(): void {
        slot.dispose();
      },
    };
  };
}

function tracked<P>(render: (props: P) => View): { component: ViewComponent<P>; calls: () => number } {
  let calls = 0;
  const component = defineView<P>((props) => {
    calls += 1;
    return render(props);
  });
  return { component, calls: () => calls };
}

type ColumnNode = { children: readonly { child: unknown }[] };

function bridgeText(view: View | undefined): string | undefined {
  if (view === undefined) return undefined;
  const node = nodeForBridge(view);
  if (node.kind !== BRIDGE_VIEW_KIND.text) throw new Error(`expected text, got ${node.kind}`);
  return node.spans[0]?.text;
}

describe.skipIf(!canRun)("T13.1 R3 — retained scope projections", () => {
  test("projection identity is stable across child content changes", () => {
    const host = new Host!(48, 12, true);
    try {
      const runtime = new RetainedExecutionRuntime({ createScopeProjection: viewSlotProjectionFactory(host) });
      let status = "ready";
      const Footer = tracked(() => composeText(`status ${status}`));
      const App = tracked(() =>
        composeVertical((column) => column.child(Footer.component({ status }))),
      );
      const root = runtime.mountRoot(App.component, {});

      // The embedded child is a component-kind node whose identity is fixed.
      const appNode = nodeForBridge(root.currentOutput!) as unknown as ColumnNode;
      expect(appNode.children[0]!.child).toBeDefined();
      const embedded = appNode.children[0]!.child as { kind: number };
      expect(embedded.kind).toBe(BRIDGE_VIEW_KIND.component);

      status = "running";
      runtime.update(root.children[0]!.scope);

      const appNodeAfter = nodeForBridge(root.currentOutput!) as unknown as ColumnNode;
      expect(appNodeAfter.children[0]!.child).toBe(embedded); // EXACT identity
      runtime.dispose();
    } finally {
      host.dispose();
    }
  });

  test("§31.3 at 3 scopes: local B update leaves A/C outputs and all ScopeRefs exact", () => {
    const host = new Host!(48, 12, true);
    try {
      const runtime = new RetainedExecutionRuntime({ createScopeProjection: viewSlotProjectionFactory(host) });
      let bValue = "b1";
      const A = tracked(() => composeText("a"));
      const B = tracked(() => composeText(`b:${bValue}`));
      const C = tracked(() => composeText("c"));
      const App = tracked(() =>
        composeVertical((column) => {
          column.child(A.component({}));
          column.child(B.component({}));
          column.child(C.component({}));
        }),
      );
      const root = runtime.mountRoot(App.component, {});
      const [scopeA, scopeB, scopeC] = root.children.map((record) => record.scope);
      const appOutputBefore = root.currentOutput;
      const aOutputBefore = scopeA!.currentOutput;
      const cOutputBefore = scopeC!.currentOutput;
      const bOutputBefore = scopeB!.currentOutput;
      const embeddedBefore = (nodeForBridge(root.currentOutput!) as unknown as ColumnNode).children.map(
        (entry) => entry.child,
      );

      bValue = "b2";
      runtime.update(scopeB!);

      // Parent composite: untouched — same OBJECT, same embedded ScopeRefs.
      expect(root.currentOutput).toBe(appOutputBefore);
      const appNodeAfter = nodeForBridge(root.currentOutput!) as unknown as ColumnNode;
      expect(appNodeAfter.children[0]!.child).toBe(embeddedBefore[0]);
      expect(appNodeAfter.children[1]!.child).toBe(embeddedBefore[1]);
      expect(appNodeAfter.children[2]!.child).toBe(embeddedBefore[2]);

      // Clean siblings: exact output identity.
      expect(scopeA!.currentOutput).toBe(aOutputBefore);
      expect(scopeC!.currentOutput).toBe(cOutputBefore);
      // Changed scope: new immutable content root.
      expect(scopeB!.currentOutput).not.toBe(bOutputBefore);
      expect(nodeForBridge(scopeB!.currentOutput!).id).not.toBe(nodeForBridge(bOutputBefore!).id);
      runtime.dispose();
    } finally {
      host.dispose();
    }
  });

  test("semantic-noop invalidation performs ZERO installs (scenario I)", () => {
    const host = new Host!(48, 12, true);
    try {
      const probes: ProjectionProbe[] = [];
      const runtime = new RetainedExecutionRuntime({
        createScopeProjection: viewSlotProjectionFactory(host, probes),
      });
      let raw = "counter 41";
      const Widget = tracked(() => composeText(raw.slice(0, 7))); // bucketed output
      const Holder = tracked(() =>
        composeVertical((column) => column.child(Widget.component({}))),
      );
      const root = runtime.mountRoot(Holder.component, {});
      expect(probes.length).toBe(1); // exactly one projected child scope
      const installsBefore = probes[0]!.installs();
      expect(installsBefore).toBe(1); // initial install at mount-commit

      raw = "counter 49"; // different raw value, SAME formatted output
      const before = executionCounterSnapshot();
      runtime.update(root.children[0]!.scope);
      const after = executionCounterSnapshot();

      // The dirty scope executed but emitted the exact previous View...
      expect(after.execution_scope_body_calls - before.execution_scope_body_calls).toBe(1);
      expect(after.execution_scope_noop_outputs - before.execution_scope_noop_outputs).toBe(1);
      expect(after.composition_exact_view_reuses - before.composition_exact_view_reuses).toBeGreaterThanOrEqual(1);
      // ...so ZERO native work happened: no re-install.
      expect(probes[0]!.installs()).toBe(installsBefore);
      runtime.dispose();
    } finally {
      host.dispose();
    }
  });

  test("failed installs keep old content authoritative on both sides", () => {
    let failInstalls = false;
    let installs = 0;
    const runtime = new RetainedExecutionRuntime({
      createScopeProjection: () => ({
        view: View.component({ id: 31 as never }),
        install(output: View): void {
          if (failInstalls) throw new Error("install refused");
          installs += 1;
          void output;
        },
        dispose(): void {},
      }),
    });

    let value = "old";
    const T = tracked(() => composeText(value));
    const Holder = tracked(() =>
      composeVertical((column) => column.child(T.component({}))),
    );
    const root = runtime.mountRoot(Holder.component, {});
    expect(installs).toBe(1); // initial install at mount-commit

    value = "new";
    failInstalls = true;
    const before = executionCounterSnapshot();
    expect(() => runtime.update(root.children[0]!.scope)).toThrow("install refused");

    // Committed world untouched: old output still authoritative.
    expect(bridgeText(root.children[0]!.scope.currentOutput)).toBe("old");
    const after = executionCounterSnapshot();
    expect(after.execution_commit_aborts - before.execution_commit_aborts).toBe(1);

    // Recovery succeeds once the failure clears.
    failInstalls = false;
    runtime.update(root.children[0]!.scope);
    expect(bridgeText(root.children[0]!.scope.currentOutput)).toBe("new");
    expect(installs).toBe(2);
    runtime.dispose();
  });

  test("detached mode (no factory) preserves R1 raw-output embedding", () => {
    const runtime = new RetainedExecutionRuntime(); // no factory
    const Footer = tracked(({ status }: { status: string }) => composeText(`footer ${status}`));
    const App = tracked(() =>
      composeVertical((column) => column.child(Footer.component({ status: "ready" }))),
    );
    const root = runtime.mountRoot(App.component, {});

    // Embedded child is the RAW text node (not a component ref).
    const appNode = nodeForBridge(root.currentOutput!) as unknown as ColumnNode;
    expect((appNode.children[0]!.child as { kind: number }).kind).toBe(BRIDGE_VIEW_KIND.text);
    runtime.dispose();
  });

  test("parent body stays unexecuted through the projection path", () => {
    const host = new Host!(48, 12, true);
    try {
      const runtime = new RetainedExecutionRuntime({ createScopeProjection: viewSlotProjectionFactory(host) });
      let status = "one";
      const Footer = tracked(() => composeText(`footer ${status}`));
      const Header = tracked(() => composeText("header"));
      const App = tracked(() =>
        composeVertical((column) => {
          column.child(Header.component({}));
          column.child(Footer.component({}));
        }),
      );
      const root = runtime.mountRoot(App.component, {});
      const appCalls = App.calls();
      const headerCalls = Header.calls();

      status = "two";
      runtime.update(root.children[1]!.scope);

      expect(App.calls()).toBe(appCalls);
      expect(Header.calls()).toBe(headerCalls);
      runtime.dispose();
    } finally {
      host.dispose();
    }
  });
});
