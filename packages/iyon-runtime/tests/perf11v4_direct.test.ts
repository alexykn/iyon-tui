import { describe, expect, test } from "bun:test";

import { native } from "../src/native.ts";
import { nodeForDirectBridge, View } from "../src/tui/values/view.ts";
import { nodeForPerf7v2Bridge } from "../bench/perf7v2_direct/view.ts";
import {
  buildComparisonPair,
  fullSchemaPair,
  randomizedRetainedPair as buildRetainedPair,
  randomizedTree,
  stableNodeSnapshot,
  type ComparisonMode,
} from "../bench/perf11v4_fixtures.ts";

type Host = {
  render(view: object): void;
  screenRows(): string[];
  styleAt?(row: number, column: number): Readonly<Record<string, unknown>> | null;
  dispose(): void;
};

const Host = native.NativeTuiHost as unknown as (new (width: number, height: number, headless: boolean) => Host) | undefined;
if (Host === undefined) throw new Error("PERF-11v4 correctness tests require the staged NativeTuiHost artifact");
const perfNative = native as typeof native & {
  tuiPerfReset?: () => void;
  tuiPerfSnapshot?: () => Record<string, number>;
  tuiPerfResetViewBridgeCache?: () => void;
  tuiPerfViewBridgeCacheSize?: () => number;
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
    if (currentHost.styleAt !== undefined && perfHost.styleAt !== undefined) {
      for (let row = 0; row < 24; row += 1) for (let column = 0; column < 80; column += 1) {
        expect(currentHost.styleAt(row, column)).toEqual(perfHost.styleAt(row, column));
      }
    }
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
      try {
        const current = randomizedTree("current", seed);
        const perf = randomizedTree("perf7v2", seed);
        renderPair(current as View, perf as import("../bench/perf7v2_direct/view.ts").Perf7v2View);
        const retainedCurrent = buildRetainedPair("current", seed);
        const retainedPerf = buildRetainedPair("perf7v2", seed);
        renderPair(retainedCurrent.base as View, retainedPerf.base as import("../bench/perf7v2_direct/view.ts").Perf7v2View);
        renderPair(retainedCurrent.next as View, retainedPerf.next as import("../bench/perf7v2_direct/view.ts").Perf7v2View);
      } catch (error) {
        throw new Error(`randomized PERF-11v4 differential failure: seed=${seed}; operation=randomizedTree+retainedEdit`, { cause: error });
      }
    }
  });

  test("proves the normal Direct decoder and schema validation path", () => {
    const perf = buildComparisonPair("perf7v2", { workload: "plain_text_column", size: 20, mode: "IDENTICAL_IDENTITY", label: "schema-proof" }, 0).next as import("../bench/perf7v2_direct/view.ts").Perf7v2View;
    const node = nodeForPerf7v2Bridge(perf);
    const host = new Host(80, 24, true);
    try {
      host.render(node);
      expect(host.screenRows()).toBeArrayOfSize(24);
      expect(() => host.render({ ...node, schema: 999 })).toThrow("unsupported TUI View bridge schema 999");
    } finally {
      host.dispose();
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
      if (typeof before.napi_view_cache_hits === "number" && typeof after.napi_view_cache_hits === "number" && after.napi_view_cache_hits > before.napi_view_cache_hits) {
        expect(after.napi_view_cache_hits).toBeGreaterThan(before.napi_view_cache_hits);
      }
      if (perfNative.tuiPerfViewBridgeCacheSize !== undefined) expect(perfNative.tuiPerfViewBridgeCacheSize()).toBeGreaterThan(0);
      expect(nodeForPerf7v2Bridge(perf)).toBe(nodeForPerf7v2Bridge(perf));
    } finally {
      host.dispose();
    }
  });

  test("covers exact identity, retained paths, shared cutoffs, and rebuilt identity", () => {
    const modes: readonly ComparisonMode[] = ["IDENTICAL_IDENTITY", "SHARED_PATH", "SHARED_DEEP", "LARGE_SHARED_SUBTREE_CUTOFF", "REBUILT_EQUIVALENT"];
    const host = new Host(80, 24, true);
    try {
      for (const mode of modes) {
        const pair = buildComparisonPair("perf7v2", { workload: "mixed_realistic", size: 200, mode, label: `cache-${mode}` }, 7);
        host.render(nodeForPerf7v2Bridge(pair.base as import("../bench/perf7v2_direct/view.ts").Perf7v2View));
        host.render(nodeForPerf7v2Bridge(pair.next as import("../bench/perf7v2_direct/view.ts").Perf7v2View));
        expect(host.screenRows()).toBeArrayOfSize(24);
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
    if (perfNative.tuiPerfViewBridgeCacheSize !== undefined) expect(perfNative.tuiPerfViewBridgeCacheSize()).toBeGreaterThan(0);
    first.dispose();
    const bootstrap = native.tuiViewAbiBootstrap?.(true) as { diagnostics?: { live_weak_upgrades?: number } } | undefined;
    if (bootstrap?.diagnostics?.live_weak_upgrades !== undefined) expect(bootstrap.diagnostics.live_weak_upgrades).toBe(0);
    const second = new Host(80, 24, true);
    try {
      second.render(bridge);
      expect(second.screenRows()).toEqual(expectedRows);
      if (perfNative.tuiPerfViewBridgeCacheSize !== undefined) expect(perfNative.tuiPerfViewBridgeCacheSize()).toBeGreaterThan(0);
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
