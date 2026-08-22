import { expect, test } from "bun:test";

import { native } from "../src/native.ts";
import {
  nativeViewRefForNodeId,
  tryNativeAxisSetChildRender,
  tryNativeAxisSpliceRender,
  tryNativeGridSetCellRender,
} from "../src/tui/native_view_abi.ts";
import { nodeForBridge, View } from "../src/tui/values/view.ts";

type StructuralHost = {
  render(view: object): void;
  screenRows(): string[];
  tuiViewAbiHostPointer(): number;
  dispose(): void;
};

const Host = native.NativeTuiHost as unknown as
  (new (width: number, height: number, headless: boolean) => StructuralHost) | undefined;

function seed(host: StructuralHost, base: View, ...children: View[]): number {
  for (const child of children) {
    host.render(nodeForBridge(child));
    if (nativeViewRefForNodeId(child) === undefined) throw new Error("native child ref unavailable");
  }
  host.render(nodeForBridge(base));
  const reference = nativeViewRefForNodeId(base);
  if (reference === undefined) throw new Error("native base ref unavailable");
  return reference;
}

/**
 * PERF-12 T4 note: the edited "next" views are constructed eagerly through
 * the public semantic API. The ruled-out packed-transport helpers are gone;
 * what is under test here are the native retained wide-edit primitives
 * (view_axis_set_child / view_axis_splice_buffer / view_grid_set_cell),
 * which PERF-12 T10 builds upon.
 */
test("native axis replace/insert/remove preserves wide host parity", () => {
  if (Host === undefined) return;
  const items = Array.from({ length: 2_048 }, (_, index) => View.text(`item-${index}`));
  const base = View.vertical(items);
  const replacement = View.text("replacement");
  const inserted = View.text("inserted");
  const withReplacement = View.vertical(items.map((item, index) => index === 1_337 ? replacement : item));
  const withInserted = View.vertical([...items.slice(0, 1_000), inserted, ...items.slice(1_000)]);
  const withRemoved = View.vertical([...items.slice(0, 1_000), ...items.slice(1_001)]);

  const cases = [
    { next: withReplacement, children: [replacement], op: (host: StructuralHost, ref: number) => tryNativeAxisSetChildRender(host, base, ref, withReplacement, replacement, 1_337) },
    { next: withInserted, children: [inserted], op: (host: StructuralHost, ref: number) => tryNativeAxisSpliceRender(host, base, ref, withInserted, 1_000, 0, [{ view: inserted }]) },
    { next: withRemoved, children: [], op: (host: StructuralHost, ref: number) => tryNativeAxisSpliceRender(host, base, ref, withRemoved, 1_000, 1, []) },
  ];

  for (const { next, children, op } of cases) {
    const host = new Host(80, 2_050, true);
    const oracle = new Host(80, 2_050, true);
    try {
      const baseRef = seed(host, base, ...children);
      const nextRef = op(host, baseRef);
      expect(nextRef).toBeDefined();
      oracle.render(nodeForBridge(next));
      expect(host.screenRows()).toEqual(oracle.screenRows());
    } finally {
      host.dispose();
      oracle.dispose();
    }
  }
});

test("native grid cell path copy preserves placement and parity", () => {
  if (Host === undefined) return;
  const base = View.grid((grid) => {
    grid.columns([{ kind: "fixed", size: 12 }, { kind: "flex" }]);
    for (let row = 0; row < 64; row += 1) {
      grid.row((cells) => {
        cells.cell(View.text(`left-${row}`));
        cells.cell(View.text(`right-${row}`));
      });
    }
  });
  const replacement = View.text("grid replacement");
  const next = View.grid((grid) => {
    grid.columns([{ kind: "fixed", size: 12 }, { kind: "flex" }]);
    for (let row = 0; row < 64; row += 1) {
      grid.row((cells) => {
        cells.cell(row === 31 ? replacement : View.text(`left-${row}`));
        cells.cell(View.text(`right-${row}`));
      });
    }
  });
  const host = new Host(80, 64, true);
  const oracle = new Host(80, 64, true);
  try {
    const baseRef = seed(host, base, replacement);
    const nextRef = tryNativeGridSetCellRender(host, base, baseRef, next, 31, 0, replacement);
    expect(nextRef).toBeDefined();
    oracle.render(nodeForBridge(next));
    expect(host.screenRows()).toEqual(oracle.screenRows());
  } finally {
    host.dispose();
    oracle.dispose();
  }
});
