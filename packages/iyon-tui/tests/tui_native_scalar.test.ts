import { describe, expect, test } from "bun:test";

import { native } from "../src/native.ts";
import { View } from "../src/index.ts";
import { AppHarness } from "../src/testing/index.ts";
import {
  NATIVE_PATH_STEP,
  NATIVE_PATH_VIEW_KIND,
  textLayoutAtNativePathForTransport,
} from "../src/api/view/view.ts";
import { nodeForBridge } from "../src/transport/structural/view-bridge.ts";

type OracleHost = {
  render(view: object): void;
  screenRows(): string[];
  dispose(): void;
};

const Host = native.NativeTuiHost as unknown as (new (width: number, height: number, headless: boolean) => OracleHost) | undefined;

describe("PERF-11.3 generated scalar retained route", () => {
  test("keeps exact identity O(1) and renders text layout through generated FFI", async () => {
    if (Host === undefined) return;
    const tui = await AppHarness.open({ width: 8, height: 4 });
    const oracle = new Host(8, 4, true);
    const base = View.text("hello");
    const changed = base.noWrap().textAlign("center");
    try {
      tui.render({ body: base });
      const first = [...tui.screenRows()];
      tui.render({ body: base });
      expect(tui.screenRows()).toEqual(first);

      tui.render({ body: changed });
      oracle.render(nodeForBridge(changed));
      expect(tui.screenRows()).toEqual(oracle.screenRows());
    } finally {
      tui.close();
      oracle.dispose();
    }
  });

  test("renders a depth-one text edit through an interned PathRef", async () => {
    if (Host === undefined) return;
    const tui = await AppHarness.open({ width: 8, height: 4 });
    const oracle = new Host(8, 4, true);
    const base = View.vertical((column) => column.child(View.text("hello")));
    const changed = textLayoutAtNativePathForTransport(
      base,
      [{ kind: NATIVE_PATH_STEP.columnChild, expectedViewKind: NATIVE_PATH_VIEW_KIND.column, selector: 0 }],
      "noWrap",
      "center",
    );
    try {
      tui.render({ body: base });
      tui.render({ body: changed });
      oracle.render(nodeForBridge(changed));
      expect(tui.screenRows()).toEqual(oracle.screenRows());
    } finally {
      tui.close();
      oracle.dispose();
    }
  });

  test("keeps depth-specialized PathRef edits parity through depth four", async () => {
    if (Host === undefined) return;
    const tui = await AppHarness.open({ width: 8, height: 4 });
    const oracle = new Host(8, 4, true);
    try {
      for (let depth = 1; depth <= 4; depth += 1) {
        let base = View.text("hello");
        const steps = [] as { kind: number; expectedViewKind: number; selector: number }[];
        for (let level = 0; level < depth; level += 1) {
          const kind = level % 2 === 0 ? NATIVE_PATH_STEP.columnChild : NATIVE_PATH_STEP.rowChild;
          const expectedViewKind = level % 2 === 0 ? NATIVE_PATH_VIEW_KIND.column : NATIVE_PATH_VIEW_KIND.row;
          base = kind === NATIVE_PATH_STEP.columnChild
            ? View.vertical((column) => column.child(base))
            : View.horizontal((row) => row.child(base));
          steps.unshift({ kind, expectedViewKind, selector: 0 });
        }
        const changed = textLayoutAtNativePathForTransport(base, steps, "noWrap", "center");
        tui.render({ body: base });
        tui.render({ body: changed });
        oracle.render(nodeForBridge(changed));
        expect(tui.screenRows()).toEqual(oracle.screenRows());
      }
    } finally {
      tui.close();
      oracle.dispose();
    }
  });

  test("renders supported root common-field patches through generated FFI", async () => {
    if (Host === undefined) return;
    const tui = await AppHarness.open({ width: 8, height: 4 });
    const oracle = new Host(8, 4, true);
    const base = View.text("x");
    const changed = base.padding(1);
    try {
      tui.render({ body: base });
      tui.render({ body: changed });
      oracle.render(nodeForBridge(changed));
      expect(tui.screenRows()).toEqual(oracle.screenRows());
    } finally {
      tui.close();
      oracle.dispose();
    }
  });
});
