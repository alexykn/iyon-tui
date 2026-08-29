import { describe, expect, test } from "bun:test";

import {
  View,
  defineView,
  state,
} from "../src/index.ts";
import { AppHarness } from "../src/testing/index.ts";
import { SEMANTIC_VIEW_KIND, semanticNodeOf } from "../src/api/view/semantic-node.ts";
import { executionCounterSnapshot, resetExecutionCounters } from "../src/composition/execution.ts";
import { lowerColdView } from "../src/transport/structural/cold-lowering.ts";
import { BRIDGE_VIEW_KIND, VIEW_BRIDGE_SCHEMA_VERSION } from "../src/transport/structural/ir.ts";

const H3B = "API-H3 H3-B composition/semantic cutover";

describe(H3B, () => {
  test("View construction is semantic-authoritative while cold transport receives a derived bridge", () => {
    const child = View.text("child");
    const view = View.vertical([child, View.text("sibling")]);
    const semantic = semanticNodeOf(view);
    const bridge = lowerColdView(view);

    expect(semantic.kind).toBe(SEMANTIC_VIEW_KIND.column);
    expect("schema" in semantic).toBe(false);
    expect(Object.isFrozen(semantic)).toBe(true);
    expect(bridge.schema).toBe(VIEW_BRIDGE_SCHEMA_VERSION);
    expect(lowerColdView(view)).toBe(bridge);
    if (semantic.kind !== SEMANTIC_VIEW_KIND.column) throw new Error("expected semantic column");
    expect(semantic.children[0]!.child).toBe(semanticNodeOf(child));
  });

  test("retained composition reuses unchanged semantic values and changes only the affected modifier", async () => {
    const value = state("ready");
    const outputs: View[] = [];
    const Body = defineView(() => {
      const output = View.text(value.value).padding(1);
      outputs.push(output);
      return output;
    });
    const tui = await AppHarness.open({ width: 20, height: 4 });
    try {
      tui.render(() => ({ body: Body({}) }));
      expect(outputs).toHaveLength(1);
      const first = outputs[0]!;
      const firstNode = semanticNodeOf(first);

      resetExecutionCounters();
      value.set("ready");
      tui.screenRows();
      expect(outputs).toHaveLength(1);
      expect(semanticNodeOf(first)).toBe(firstNode);
      const noOpCounters = executionCounterSnapshot();
      expect(noOpCounters.execution_scope_body_calls).toBe(0);
      expect(noOpCounters.composition_new_views).toBe(0);

      resetExecutionCounters();
      value.set("changed");
      tui.screenRows();
      expect(outputs).toHaveLength(2);
      const second = outputs[1]!;
      expect(second).not.toBe(first);
      expect(semanticNodeOf(second)).not.toBe(firstNode);
      expect(tui.screenRows().join("\n")).toContain("changed");
    } finally {
      tui.close();
    }
  });

  test("component semantic identity is the local HandleId and transport resolves it only at lowering", async () => {
    const tui = await AppHarness.open({ width: 20, height: 4 });
    const slot = tui.createViewSlot(View.text("seed"));
    try {
      const view = slot.view();
      const semantic = semanticNodeOf(view);
      expect(semantic.kind).toBe(SEMANTIC_VIEW_KIND.component);
      if (semantic.kind !== SEMANTIC_VIEW_KIND.component) throw new Error("expected semantic component");
      expect(semantic.handleId).toBe(slot.id);
      expect("handle" in semantic).toBe(false);

      const bridge = lowerColdView(view);
      if (bridge.kind !== BRIDGE_VIEW_KIND.component) throw new Error("expected bridge component");
      expect(bridge.handle).toBeGreaterThan(0);
      slot.dispose();
      expect(() => slot.view()).toThrow(/disposed/i);
    } finally {
      tui.close();
    }
  });
});
