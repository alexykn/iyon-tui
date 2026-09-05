import { expect, test } from "bun:test";

import { View } from "../src/index.ts";
import { StyleRef } from "../src/index.ts";
import { AppHarness } from "../src/testing/index.ts";

const ENVELOPE = "L1-05 typed retained-state envelope";

test(`${ENVELOPE} applies geometry overrides and reveals base on clear`, async () => {
  const tui = await AppHarness.open({ width: 12, height: 5 });
  const state = tui.viewState();
  try {
    tui.render(() => ({ body: View.text("hi").state(state) }));
    const base = tui.screenRows();
    expect(base.join("\n")).toContain("hi");

    state.setGeometry({ alignment: "center" });
    tui.flush();
    const centered = tui.screenRows();
    expect(centered).not.toEqual(base);
    expect(centered.join("\n")).toContain("hi");

    state.clearGeometry("alignment");
    tui.flush();
    expect(tui.screenRows()).toEqual(base);

    state.setGeometry({ padding: 1, minWidth: null });
    tui.flush();
    expect(tui.screenRows()).not.toEqual(base);
    state.clearGeometry();
    tui.flush();
    expect(tui.screenRows()).toEqual(base);
  } finally {
    tui.close();
  }
});

test(`${ENVELOPE} packs the full geometry envelope on an axis`, async () => {
  const tui = await AppHarness.open({ width: 16, height: 8 });
  const state = tui.viewState();
  try {
    tui.render(() => ({
      body: View.horizontal([View.text("one"), View.text("two")]).state(state),
    }));
    const base = tui.screenRows();
    // Every geometry lane in one envelope: enums, insets, nullable bounds,
    // gap, object alignment, edge object, and null.
    state.setGeometry({
      width: "fill",
      height: "fit",
      padding: { top: 1, right: 2, bottom: 0, left: 0 },
      minWidth: 1,
      maxWidth: 16,
      minHeight: null,
      maxHeight: 7,
      gap: 1,
      alignment: { vertical: "top" },
      borderEdges: { top: true, right: false, bottom: true, left: false },
    });
    tui.flush();
    expect(tui.screenRows()).not.toEqual(base);
    state.clearGeometry("gap", "alignment", "borderEdges");
    tui.flush();
    state.setGeometry({ borderEdges: "all" });
    state.setGeometry({ borderEdges: "topBottom" });
    state.setGeometry({ borderEdges: null });
    tui.flush();
    state.clearGeometry();
    tui.flush();
    expect(tui.screenRows()).toEqual(base);
  } finally {
    tui.close();
  }
});

test(`${ENVELOPE} rejects invalid patches without partial application`, async () => {
  const tui = await AppHarness.open({ width: 12, height: 5 });
  const state = tui.viewState();
  try {
    tui.render(() => ({ body: View.text("hi").state(state) }));
    state.setGeometry({ padding: 2 });
    tui.flush();
    const applied = tui.screenRows();
    expect(applied).not.toEqual(tui.screenRows().map(() => "            "));

    expect(() => state.setGeometry({ padding: 1, gap: -1 })).toThrow(
      "ViewState gap must be an integer from 0 to 65535",
    );
    tui.flush();
    expect(tui.screenRows()).toEqual(applied);

    expect(() => state.setGeometry({ padding: 1 })).not.toThrow();
    state.clearGeometry("padding");
    tui.flush();
  } finally {
    tui.close();
  }
});

test(`${ENVELOPE} preserves geometry validation errors`, async () => {
  const tui = await AppHarness.open({ width: 12, height: 3 });
  const state = tui.viewState();
  try {
    tui.render(() => ({ body: View.text("hi").state(state) }));
    expect(() => state.setGeometry({ bogus: 1 } as never)).toThrow(
      'unknown ViewState geometry property "bogus"',
    );
    expect(() => state.setGeometry({ width: "wide" } as never)).toThrow(
      "ViewState width must be fit or fill",
    );
    expect(() => state.setGeometry({ gap: 1.5 })).toThrow(
      "ViewState gap must be an integer from 0 to 65535",
    );
    expect(() => state.setGeometry({ minWidth: "x" } as never)).toThrow(
      "ViewState minWidth must be an integer from 0 to 65535",
    );
    expect(() => state.setGeometry({ padding: null } as never)).toThrow(
      "ViewState padding must be Insets or an InsetsValue",
    );
    expect(() => state.setGeometry({ alignment: "sideways" } as never)).toThrow(
      "ViewState alignment must be a known alignment or an alignment object",
    );
    expect(() => state.setGeometry({ alignment: {} })).toThrow(
      "ViewState alignment must specify an axis",
    );
    expect(() => state.setGeometry({ alignment: { horizontal: "up" } } as never)).toThrow(
      "ViewState horizontal alignment is invalid",
    );
    expect(() => state.setGeometry({ borderEdges: 5 } as never)).toThrow(
      "ViewState borderEdges must be all, topBottom, an edge object, or null",
    );
    expect(() => state.setGeometry({ borderEdges: { top: 1 } } as never)).toThrow(
      'ViewState border edge "top" must be boolean',
    );
    expect(() => state.setGeometry({ borderEdges: { sideways: true } } as never)).toThrow(
      'unknown ViewState border edge "sideways"',
    );
    // undefined values are omitted, not packed.
    expect(() => state.setGeometry({ width: undefined, minWidth: 2 })).not.toThrow();
    state.clearGeometry("minWidth");
    // Clear-list validation is unchanged.
    expect(() => state.clearGeometry("bogus" as never)).toThrow(
      'unknown ViewState geometry clear property "bogus"',
    );
    expect(() => state.clearGeometry("padding", "padding")).toThrow(
      'duplicate ViewState geometry clear property "padding"',
    );
  } finally {
    tui.close();
  }
});

test(`${ENVELOPE} preserves presentation validation errors`, async () => {
  const tui = await AppHarness.open({ width: 12, height: 3 });
  const state = tui.viewState();
  try {
    tui.render(() => ({ body: View.text("hi").state(state) }));
    expect(() => state.setPresentation({ bogus: 1 } as never)).toThrow(
      'unknown ViewState presentation property "bogus"',
    );
    expect(() => state.setPresentation({ borderStyle: "dashed" } as never)).toThrow(
      'unknown ViewState border style "dashed"',
    );
    expect(() => state.setPresentation({ borderGlyphs: { top: 1 } } as never)).toThrow(
      'ViewState border glyph "top" must be a string',
    );
    expect(() => state.setPresentation({ borderGlyphs: { bogus: "x" } } as never)).toThrow(
      'unknown ViewState border glyph "bogus"',
    );
    expect(() => state.setPresentation({ textAttributes: { bogus: true } } as never)).toThrow(
      'unknown text attribute "bogus"',
    );
    expect(() => state.setPresentation({ textAttributes: { bold: "yes" } } as never)).toThrow(
      'ViewState text attribute "bold" must be boolean',
    );
    expect(() => state.setPresentation({ textAttributes: null } as never)).toThrow(
      "ViewState textAttributes must be an object",
    );
    expect(() => state.clearPresentation("bogus" as never)).toThrow(
      'unknown ViewState presentation property "bogus"',
    );
    expect(() => state.clearPresentation("style", "style")).toThrow(
      'duplicate ViewState clear property "style"',
    );
    // A full presentation envelope still applies through the new path.
    expect(() =>
      state.setPresentation({
        borderStyle: "rounded",
        borderGlyphs: {
          top: "-",
          right: "|",
          bottom: "-",
          left: "|",
          topLeft: "+",
          topRight: "+",
          bottomLeft: "+",
          bottomRight: "+",
        },
        textAttributes: { bold: true },
        style: StyleRef.theme("diff.meta"),
      }),
    ).not.toThrow();
    state.clearPresentation();
    tui.flush();
  } finally {
    tui.close();
  }
});
