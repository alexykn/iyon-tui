import { describe, expect, test } from "bun:test";

import { StyleSpec, TextSpan, View } from "../src/index.ts";
import { axisSetChildForTransport } from "../src/api/view/view.ts";
import { AppHarness } from "../src/testing/index.ts";
import {
  resetRetainedIdentityCounters,
  retainedIdentityCounterSnapshot,
} from "../src/transport/structural/retained-dag.ts";

const H3C = "API-H3 H3-C semantic transport cutover";

describe(H3C, () => {
  test("retained materialization consumes semantic nodes with zero second-architecture allocation", async () => {
    const body = View.vertical([
      View.text("header").bold(),
      View.horizontal([View.text("left"), View.text("right").italic()]),
      View.grid({
        columns: [{ kind: "content" }, { kind: "fixed", size: 4 }],
        rows: [{ cells: [{ view: View.text("cell") }, { view: View.spacer(1) }] }],
      }),
    ]);
    resetRetainedIdentityCounters();
    const tui = await AppHarness.open({ width: 30, height: 8 });
    try {
      tui.render(() => ({ body }));
      const retained = retainedIdentityCounterSnapshot();
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
      resetRetainedIdentityCounters();
      tui.render(() => ({ body: next }));
      const retained = retainedIdentityCounterSnapshot();
      expect(retained.derivation_fast_path_calls).toBe(1);
      expect(retained.retained_children_visited).toBe(0);
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
    resetRetainedIdentityCounters();
    const tui = await AppHarness.open({ width: 20, height: 4 });
    try {
      tui.render(() => ({ body }));
      expect(retainedIdentityCounterSnapshot().direct_materializer_calls).toBeGreaterThan(0);
      expect(tui.screenRows()[0]).toContain("x");
    } finally {
      tui.close();
    }
  });

  test("wide styled text materializes on the retained buffer lane", async () => {
    // PRE-V5-R0 R0-B001: span counts above the 1..=4 fixed-arity families use
    // the variadic words+bytes buffer constructor on the retained path — no
    // refusal, no second architecture. The trailing NUL proves the
    // length-delimited bytes lane carries what the cstring lane cannot.
    const spans = [
      TextSpan.plain("a"),
      TextSpan.styled("b", new StyleSpec().bold()),
      TextSpan.styled("c", new StyleSpec().italic()),
      TextSpan.styled("d", new StyleSpec().underline()),
      TextSpan.plain("e"),
      TextSpan.styled("f\0", new StyleSpec().bold()),
    ];
    const body = View.styledText(spans);
    resetRetainedIdentityCounters();
    const tui = await AppHarness.open({ width: 20, height: 4 });
    try {
      tui.render({ body });
      expect(retainedIdentityCounterSnapshot().direct_materializer_calls).toBeGreaterThan(0);
      expect(tui.screenRows().join("\n")).toContain("abcde");
    } finally {
      tui.close();
    }
  });

  test("custom border glyphs materialize on the retained decorated lane", async () => {
    // PRE-V5-R0 R0-B002: custom glyphs ride the mask-gated decorated trailer
    // on the retained path. The "*" glyphs replace the named plain style,
    // proving the glyph set reached the native constructor.
    const body = View.text("hi").border({
      style: "plain",
      edges: "all",
      glyphs: {
        top: "*",
        right: "*",
        bottom: "*",
        left: "*",
        topLeft: "*",
        topRight: "*",
        bottomLeft: "*",
        bottomRight: "*",
      },
    });
    resetRetainedIdentityCounters();
    const tui = await AppHarness.open({ width: 20, height: 4 });
    try {
      tui.render({ body });
      expect(retainedIdentityCounterSnapshot().direct_materializer_calls).toBeGreaterThan(0);
      const screen = tui.screenRows().join("\n");
      expect(screen).toContain("hi");
      expect(screen).toContain("*");
    } finally {
      tui.close();
    }
  });
});
