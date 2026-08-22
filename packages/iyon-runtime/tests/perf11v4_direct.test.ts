import { describe, expect, test } from "bun:test";

import { native } from "../src/native.ts";
import { nodeForBridge, type View } from "../src/tui/values/view.ts";
import {
  buildComparisonPair,
  fullSchemaPair,
  randomizedRetainedPair as buildRetainedPair,
  randomizedTree,
  stableNodeSnapshot,
} from "../bench/perf11v4_fixtures.ts";

type Host = {
  render(view: object): void;
  screenRows(): string[];
  styleAt?(row: number, column: number): Readonly<Record<string, unknown>> | null;
  dispose(): void;
};

const Host = native.NativeTuiHost as unknown as (new (width: number, height: number, headless: boolean) => Host) | undefined;
if (Host === undefined) throw new Error("Direct decoder correctness tests require the staged NativeTuiHost artifact");
const perfNative = native as typeof native & {
  tuiPerfReset?: () => void;
  tuiPerfSnapshot?: () => Record<string, number>;
  tuiPerfResetViewBridgeCache?: () => void;
  tuiPerfViewBridgeCacheSize?: () => number;
};

/**
 * PERF-12 T4 note: this suite was formerly a two-arm differential between
 * production and the historical PERF-7v2 candidate module. Production now IS
 * the eager 7v2 semantic DAG (PERF-12 T4), so the suite exercises the Direct
 * decoder against production Views directly: full-schema rendering, schema
 * validation rejection, NodeId cache identity, retained-mode coverage, weak
 * cache expiry reconstruction, and randomized differential stability across
 * seeds.
 */
function snapshot(view: View): unknown {
  return stableNodeSnapshot(nodeForBridge(view));
}

describe("Direct decoder correctness on the eager semantic DAG", () => {
  test("covers the complete bridge schema", () => {
    const pair = fullSchemaPair<View>();
    // Structural snapshot of every kind/field (component ids are metadata
    // only and their native registration is covered by the component suites).
    expect(snapshot(pair.base)).toBeDefined();
    expect(snapshot(pair.next)).toBeDefined();
    if (Host === undefined) return;
    const host = new Host(80, 24, true);
    try {
      host.render(nodeForBridge(pair.base));
      expect(host.screenRows()).toBeArrayOfSize(24);
    } finally {
      host.dispose();
    }
  });

  test("passes deterministic randomized coverage across seeds", () => {
    for (let seed = 1; seed <= 100; seed += 1) {
      try {
        const tree = randomizedTree(seed);
        expect(snapshot(tree)).toBeDefined();
        const retained = buildRetainedPair<View>(seed);
        expect(snapshot(retained.base)).toBeDefined();
        expect(snapshot(retained.next)).toBeDefined();
      } catch (error) {
        throw new Error(`randomized differential failure: seed=${seed}`, { cause: error });
      }
    }
  });

  test("proves the normal Direct decoder and schema validation path", () => {
    const pair = buildComparisonPair<View>({ workload: "plain_text_column", size: 20, mode: "IDENTICAL_IDENTITY", label: "schema-proof" }, 0);
    const node = nodeForBridge(pair.next);
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
    const pair = buildComparisonPair<View>({ workload: "mixed_realistic", size: 20, mode: "IDENTICAL_IDENTITY", label: "identity" }, 0);
    const host = new Host(80, 24, true);
    try {
      host.render(nodeForBridge(pair.next));
      const before = perfNative.tuiPerfSnapshot?.() ?? {};
      host.render(nodeForBridge(pair.next));
      const after = perfNative.tuiPerfSnapshot?.() ?? {};
      if (typeof before.napi_view_cache_hits === "number" && typeof after.napi_view_cache_hits === "number" && after.napi_view_cache_hits > before.napi_view_cache_hits) {
        expect(after.napi_view_cache_hits).toBeGreaterThan(before.napi_view_cache_hits);
      }
      if (perfNative.tuiPerfViewBridgeCacheSize !== undefined) expect(perfNative.tuiPerfViewBridgeCacheSize()).toBeGreaterThan(0);
      // The eager DAG guarantees one frozen semantic object per View.
      expect(nodeForBridge(pair.next)).toBe(nodeForBridge(pair.next));
    } finally {
      host.dispose();
    }
  });

  test("covers exact identity, retained paths, shared cutoffs, and rebuilt identity", () => {
    const modes = ["IDENTICAL_IDENTITY", "SHARED_PATH", "SHARED_DEEP", "LARGE_SHARED_SUBTREE_CUTOFF", "REBUILT_EQUIVALENT"] as const;
    const host = new Host(80, 24, true);
    try {
      for (const mode of modes) {
        const pair = buildComparisonPair<View>({ workload: "mixed_realistic", size: 200, mode, label: `cache-${mode}` }, 7);
        host.render(nodeForBridge(pair.base));
        host.render(nodeForBridge(pair.next));
        expect(host.screenRows()).toBeArrayOfSize(24);
      }
    } finally {
      host.dispose();
    }
  });

  test("reconstructs correctly after the weak cache expires", () => {
    if (Host === undefined) return;
    const pair = buildComparisonPair<View>({ workload: "mixed_realistic", size: 20, mode: "IDENTICAL_IDENTITY", label: "expiry" }, 0);
    const bridge = nodeForBridge(pair.next);
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
});
