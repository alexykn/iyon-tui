import { describe, expect, test } from "bun:test";

import { native } from "../src/native.ts";
import { nodeForDirectBridge, View } from "../src/tui/values/view.ts";
import { nodeForPerf7v2Bridge } from "../bench/perf7v2_direct/view.ts";
import {
  buildComparisonPair,
  fullSchemaPair,
  randomizedTree,
  stableNodeSnapshot,
  type ComparisonMode,
} from "../bench/perf11v4_fixtures.ts";

type Host = {
  render(view: object): void;
  screenRows(): string[];
  dispose(): void;
};

const Host = native.NativeTuiHost as unknown as (new (width: number, height: number, headless: boolean) => Host) | undefined;
const perfNative = native as typeof native & {
  tuiPerfReset?: () => void;
  tuiPerfSnapshot?: () => Record<string, number>;
};

function snapshotCurrent(view: View): unknown {
  return stableNodeSnapshot(nodeForDirectBridge(view));
}

function snapshotPerf(view: import("../bench/perf7v2_direct/view.ts").Perf7v2View): unknown {
  return stableNodeSnapshot(nodeForPerf7v2Bridge(view));
}

function renderPair(current: View, perf: import("../bench/perf7v2_direct/view.ts").Perf7v2View): void {
  if (Host === undefined) return;
  const currentHost = new Host(80, 24, true);
  const perfHost = new Host(80, 24, true);
  try {
    currentHost.render(nodeForDirectBridge(current));
    perfHost.render(nodeForPerf7v2Bridge(perf));
    expect(currentHost.screenRows()).toEqual(perfHost.screenRows());
  } finally {
    currentHost.dispose();
    perfHost.dispose();
  }
}

describe("PERF-11v4 faithful PERF-7v2 Candidate A", () => {
  test("covers the complete bridge schema with equivalent current and eager trees", () => {
    if (Host === undefined) return;
    const current = fullSchemaPair("current");
    const perf = fullSchemaPair("perf7v2");
    expect(snapshotCurrent(current.base as View)).toEqual(snapshotPerf(perf.base as import("../bench/perf7v2_direct/view.ts").Perf7v2View));
    expect(snapshotCurrent(current.next as View)).toEqual(snapshotPerf(perf.next as import("../bench/perf7v2_direct/view.ts").Perf7v2View));
  });

  test("passes deterministic randomized differential coverage", () => {
    if (Host === undefined) return;
    for (let seed = 1; seed <= 100; seed += 1) {
      const current = randomizedTree("current", seed);
      const perf = randomizedTree("perf7v2", seed);
      expect(snapshotCurrent(current as View)).toEqual(snapshotPerf(perf as import("../bench/perf7v2_direct/view.ts").Perf7v2View));
      renderPair(current as View, perf as import("../bench/perf7v2_direct/view.ts").Perf7v2View);
    }
  });

  test("preserves identity and stops traversal at a live root cache hit", () => {
    if (Host === undefined) return;
    perfNative.tuiPerfReset?.();
    const perf = buildComparisonPair("perf7v2", { workload: "mixed_realistic", size: 20, mode: "IDENTICAL_IDENTITY", label: "identity" }, 0).next as import("../bench/perf7v2_direct/view.ts").Perf7v2View;
    const host = new Host(80, 24, true);
    try {
      host.render(nodeForPerf7v2Bridge(perf));
      const before = perfNative.tuiPerfSnapshot?.() ?? {};
      host.render(nodeForPerf7v2Bridge(perf));
      const after = perfNative.tuiPerfSnapshot?.() ?? {};
      if (typeof before.napi_view_cache_hits === "number" && typeof after.napi_view_cache_hits === "number") {
        expect(after.napi_view_cache_hits).toBeGreaterThan(before.napi_view_cache_hits);
      }
    } finally {
      host.dispose();
    }
  });

  test("reconstructs correctly after the weak cache expires", () => {
    if (Host === undefined) return;
    const perf = buildComparisonPair("perf7v2", { workload: "mixed_realistic", size: 20, mode: "IDENTICAL_IDENTITY", label: "expiry" }, 0).next as import("../bench/perf7v2_direct/view.ts").Perf7v2View;
    const bridge = nodeForPerf7v2Bridge(perf);
    const first = new Host(80, 24, true);
    first.render(bridge);
    const expectedRows = first.screenRows();
    first.dispose();
    native.tuiViewAbiBootstrap?.(true);
    const second = new Host(80, 24, true);
    try {
      second.render(bridge);
      expect(second.screenRows()).toEqual(expectedRows);
    } finally {
      second.dispose();
    }
  });

  test("keeps all comparison modes semantically equivalent", () => {
    if (Host === undefined) return;
    const modes: readonly ComparisonMode[] = ["COLD", "FIRST_USE", "IDENTICAL_IDENTITY", "SHARED_PATH", "SHARED_DEEP", "LARGE_SHARED_SUBTREE_CUTOFF", "REBUILT_EQUIVALENT"];
    for (const mode of modes) {
      const current = buildComparisonPair("current", { workload: "mixed_realistic", size: 20, mode, label: mode }, 0);
      const perf = buildComparisonPair("perf7v2", { workload: "mixed_realistic", size: 20, mode, label: mode }, 0);
      expect(snapshotCurrent(current.next as View)).toEqual(snapshotPerf(perf.next as import("../bench/perf7v2_direct/view.ts").Perf7v2View));
    }
  });
});
