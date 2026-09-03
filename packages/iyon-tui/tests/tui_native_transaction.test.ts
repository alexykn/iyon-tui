import { describe, expect, test } from "bun:test";

import { native } from "../src/transport/native/addon.ts";
import {
  nativeViewAbiSession,
  nativeViewRefForNodeId,
  releaseNativeViewRef,
  tryRetainedEditTransactionRender,
} from "../src/transport/structural/native-view-abi.ts";
import { viewNodeId, View } from "../src/api/view/view.ts";
import {
  NATIVE_PATH_STEP,
  NATIVE_PATH_VIEW_KIND,
  nativePathChildLineage,
} from "../src/transport/structural/retained-path.ts";
import type { NativeTuiHostContract } from "../src/transport/native/addon.ts";
import { renderRetained } from "./fixtures/native-host.ts";

type HostContract = NativeTuiHostContract;

const Host = native.NativeTuiHost as unknown as (new (width: number, height: number, headless: boolean) => HostContract) | undefined;

describe("PERF-11.6 native edit transactions", () => {
  test("shares a changed root across two typed text edits and preserves host parity", () => {
    if (Host === undefined || nativeViewAbiSession() === undefined) return;
    const host = new Host(16, 4, true);
    const reference = new Host(16, 4, true);
    const left = View.text("left");
    const right = View.text("right");
    const base = View.vertical((column) => {
      column.child(left);
      column.child(right);
    });
    const changedLeft = left.noWrap();
    const changedRight = right.textAlign("center");
    const changed = View.vertical((column) => {
      column.child(changedLeft);
      column.child(changedRight);
    });
    try {
      renderRetained(host, base);
      const baseRef = nativeViewRefForNodeId(base);
      if (baseRef === undefined) return;
      const leftLineage = nativePathChildLineage(base, undefined, {
        kind: NATIVE_PATH_STEP.columnChild,
        expectedViewKind: NATIVE_PATH_VIEW_KIND.column,
        selector: 0,
      });
      const rightLineage = nativePathChildLineage(base, undefined, {
        kind: NATIVE_PATH_STEP.columnChild,
        expectedViewKind: NATIVE_PATH_VIEW_KIND.column,
        selector: 1,
      });
      const result = tryRetainedEditTransactionRender(host, base, baseRef, [
        { lineage: leftLineage, nodeIds: [viewNodeId(changedLeft), viewNodeId(changed)], wrap: 3, align: 1 },
        { lineage: rightLineage, nodeIds: [viewNodeId(changedRight), viewNodeId(changed)], wrap: 2, align: 2 },
      ]);
      expect(result).toBeGreaterThan(0);
      renderRetained(reference, changed);
      expect(host.screenRows()).toEqual(reference.screenRows());
      releaseNativeViewRef(nativeViewAbiSession(), result!);
    } finally {
      host.dispose();
      reference.dispose();
    }
  });
});
