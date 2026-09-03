import { describe, expect, test } from "bun:test";

import { TextSpan, View } from "../src/index.ts";
import { axisSetChildForTransport } from "../src/api/view/view.ts";
import { AppHarness } from "../src/testing/index.ts";
import {
  coldLoweringCounterSnapshot,
  resetColdLoweringCounters,
} from "../src/transport/structural/cold-lowering.ts";
import {
  resetRetainedIdentityCounters,
  retainedIdentityCounterSnapshot,
} from "../src/transport/structural/retained-dag.ts";

const H3C = "API-H3 H3-C semantic transport cutover";

describe(H3C, () => {
  test("retained materialization consumes semantic nodes without cold bridge allocation", async () => {
    const body = View.vertical([
      View.text("header").bold(),
      View.horizontal([View.text("left"), View.text("right").italic()]),
      View.grid({
        columns: [{ kind: "content" }, { kind: "fixed", size: 4 }],
        rows: [{ cells: [{ view: View.text("cell") }, { view: View.spacer(1) }] }],
      }),
    ]);
    resetColdLoweringCounters();
    resetRetainedIdentityCounters();
    const tui = await AppHarness.open({ width: 30, height: 8 });
    try {
      tui.render(() => ({ body }));
      const cold = coldLoweringCounterSnapshot();
      const retained = retainedIdentityCounterSnapshot();
      expect(cold.cold_bridge_objects_allocated).toBe(0);
      expect(retained.direct_materializer_calls).toBeGreaterThan(0);
      expect(tui.screenRows().join("\n")).toContain("header");
    } finally {
      tui.close();
    }
  });

  test("wide semantic sequences stay lazy on the retained edit path", async () => {
    const base = View.vertical(
      Array.from({ length: 2_000 }, (_, index) => View.text(`row-${index}`).noWrap()),
    );
    const next = axisSetChildForTransport(base, 0, View.text("replacement").noWrap());
    const tui = await AppHarness.open({ width: 20, height: 8 });
    try {
      // The initial 2,000-child root materializes through the single retained
      // path (PRE-V5-R0: no arity refusal; the axis buffer carries all
      // children and the native limit is the only bound).
      tui.render(() => ({ body: base }));
      resetColdLoweringCounters();
      resetRetainedIdentityCounters();
      tui.render(() => ({ body: next }));
      const cold = coldLoweringCounterSnapshot();
      const retained = retainedIdentityCounterSnapshot();
      expect(cold.cold_bridge_objects_allocated).toBe(0);
      expect(retained.derivation_fast_path_calls).toBe(1);
      expect(retained.bridge_children_visited).toBe(0);
      expect(tui.screenRows().join("\n")).toContain("replacement");
    } finally {
      tui.close();
    }
  });

  test("retained component lowering resolves HandleId to the live native component", async () => {
    const tui = await AppHarness.open({ width: 20, height: 4 });
    const slot = tui.createViewSlot(View.text("component-seed"));
    try {
      slot.setView(View.text("component-content"));
      tui.render({ body: slot.view() });
      expect(tui.screenRows().some((row) => row.includes("component-content"))).toBe(true);
    } finally {
      tui.close();
    }
  });

  test("large NUL-bearing text stays on the retained path", async () => {
    // PRE-V5-R0: the exact-byte lane carries payloads up to the native text
    // limit. No size heuristic selects another transport.
    const body = View.text(`${"x".repeat(65_536)}\0`);
    resetColdLoweringCounters();
    resetRetainedIdentityCounters();
    const tui = await AppHarness.open({ width: 20, height: 4 });
    try {
      tui.render(() => ({ body }));
      expect(coldLoweringCounterSnapshot().cold_bridge_objects_allocated).toBe(0);
      expect(retainedIdentityCounterSnapshot().direct_materializer_calls).toBeGreaterThan(0);
      expect(tui.screenRows()[0]).toContain("x");
    } finally {
      tui.close();
    }
  });

  test("unsupported shapes fail explicitly instead of selecting another transport", async () => {
    // PRE-V5-R0 BLOCKER R0-B001: 5-span styled text has no retained
    // constructor. The refusal is an explicit preparation failure; the
    // previous visible frame stays authoritative and no bridge object is
    // allocated for a second architecture.
    const spans = ["a", "b", "c", "d", "e"].map((text) => TextSpan.plain(text));
    const body = View.styledText(spans);
    resetColdLoweringCounters();
    const tui = await AppHarness.open({ width: 20, height: 4 });
    try {
      expect(() => tui.render({ body })).toThrow("TUI_ROOT_PREPARATION_FAILED");
      expect(coldLoweringCounterSnapshot().cold_bridge_objects_allocated).toBe(0);
    } finally {
      tui.close();
    }
  });
});
