import { describe, expect, test } from "bun:test";

import { native } from "../src/native.ts";
import { nativeViewAbiSession } from "../src/tui/native_view_abi.ts";
import { BRIDGE_VIEW_KIND } from "../src/tui/ir.ts";
import { nodeForBridge, View } from "../src/tui/values/view.ts";
import {
  retainedIdentityCounterSnapshot,
  RetainedRootBoundary,
} from "../src/tui/retained_dag.ts";
import {
  persistentSeqCounters,
  resetPersistentSeqCounters,
} from "../src/tui/persistent_seq.ts";

type Host = {
  render(view: object): void;
  screenRows(): string[];
  tuiViewAbiHostPointer(): number;
  dispose(): void;
};

const Host = native.NativeTuiHost as unknown as
  | (new (width: number, height: number, headless: boolean) => Host)
  | undefined;
const session = nativeViewAbiSession();
const canRun = Host !== undefined && session !== undefined;

function wideColumn(width: number): View {
  return View.vertical(Array.from({ length: width }, (_, index) => View.spacer((index % 3) + 1)));
}

function renderOracle(view: View, width: number, height: number): string[] {
  const host = new Host!(width, height, true);
  try {
    host.render(nodeForBridge(view));
    return host.screenRows();
  } finally {
    host.dispose();
  }
}

function installWithBoundary(view: View, previous: View, width: number, height: number): {
  boundary: RetainedRootBoundary;
  host: Host;
} {
  const host = new Host!(width, height, true);
  host.render(nodeForBridge(previous));
  const boundary = new RetainedRootBoundary(session!, () => host.tuiViewAbiHostPointer() as never);
  if (!boundary.adopt(previous)) throw new Error("T10 boundary adoption failed");
  return { boundary, host };
}

describe("PERF-12 T10 wide retained edits", () => {
  test("§35: one-child replacement sends base ref + index + child ref, not the old sequence", () => {
    if (!canRun) return;
    const base = wideColumn(2_000);
    const child = View.spacer(7);
    const next = View.axisSetChildForTransport(base, 1_234, child);
    const nextNode = nodeForBridge(next);
    expect(nextNode.kind).toBeDefined();

    const { boundary, host } = installWithBoundary(base, base, 80, 24);
    try {
      const before = retainedIdentityCounterSnapshot();
      expect(boundary.install(next)).toBeGreaterThan(0);
      const after = retainedIdentityCounterSnapshot();
      expect(after.derivation_fast_path_calls - before.derivation_fast_path_calls).toBe(1);
      // Only the replacement leaf is a direct materialization; the 2,000
      // unchanged children never cross FFI or get inspected.
      expect(after.direct_materializer_calls - before.direct_materializer_calls).toBe(1);
      expect(after.bridge_children_visited - before.bridge_children_visited).toBe(0);
      expect(after.ref_words_written - before.ref_words_written).toBe(0);
      expect(host.screenRows()).toEqual(renderOracle(next, 80, 24));
    } finally {
      boundary.close();
      host.dispose();
    }
  });

  test("§35: insert, remove, and splice preserve exact order and render parity", () => {
    if (!canRun) return;
    const base = wideColumn(2_000);
    const inserted = View.spacer(9);
    const cases = [
      View.axisSpliceForTransport(base, 700, 0, [{ view: inserted }]),
      View.axisSpliceForTransport(base, 700, 1, []),
      View.axisSpliceForTransport(base, 700, 4, [
        { view: View.spacer(4) },
        { view: View.spacer(5) },
        { view: View.spacer(6) },
        { view: View.spacer(7) },
      ]),
    ];
    for (const next of cases) {
      const { boundary, host } = installWithBoundary(base, base, 80, 24);
      try {
        const before = retainedIdentityCounterSnapshot();
        expect(boundary.install(next)).toBeGreaterThan(0);
        const after = retainedIdentityCounterSnapshot();
        expect(after.derivation_fast_path_calls - before.derivation_fast_path_calls).toBe(1);
        expect(after.bridge_children_visited - before.bridge_children_visited).toBe(0);
        expect(host.screenRows()).toEqual(renderOracle(next, 80, 24));
      } finally {
        boundary.close();
        host.dispose();
      }
    }
  });

  test("§34/§96: 2k/10k/100k set edits clone logarithmic PersistentSeq work", () => {
    for (const width of [2_000, 10_000, 100_000]) {
      const base = wideColumn(width);
      resetPersistentSeqCounters();
      const next = View.axisSetChildForTransport(base, width >> 1, View.spacer(3));
      const snapshot = { ...persistentSeqCounters };
      // set clones height + leaf, never width: generous bound remains below
      // one branch factor even at 100k and is comparable across widths.
      expect(snapshot.branches_cloned).toBeLessThanOrEqual(8);
      expect(snapshot.items_iterated).toBeLessThanOrEqual(64);
      expect(snapshot.nodes_cloned).toBeLessThanOrEqual(10);
      // Flat children are intentionally absent from the retained edit until
      // a fallback/Direct consumer asks for them.
      const beforeAccess = { ...persistentSeqCounters };
      const node = nodeForBridge(next);
      expect(node.kind === BRIDGE_VIEW_KIND.row || node.kind === BRIDGE_VIEW_KIND.column).toBe(true);
      expect(persistentSeqCounters.items_iterated).toBe(beforeAccess.items_iterated);
      if (node.kind === BRIDGE_VIEW_KIND.row || node.kind === BRIDGE_VIEW_KIND.column) {
        expect(node.children.length).toBe(width);
      }
      expect(persistentSeqCounters.items_iterated).toBeGreaterThanOrEqual(beforeAccess.items_iterated);
    }
  });

  test("§36: new Grid uses the borrowed word lane and retained cell edit", () => {
    if (!canRun) return;
    const cellA = View.spacer(1);
    const cellB = View.spacer(2);
    const base = View.grid({
      columns: [{ kind: "content" }, { kind: "fixed", size: 3 }],
      rows: [
        { track: { kind: "content" }, cells: [{ view: cellA }, { view: cellB }] },
      ],
      columnGap: 1,
      rowGap: 1,
    });
    const replacement = View.spacer(8);
    const next = View.gridSetCellForTransport(base, 0, 1, replacement);
    const { boundary, host } = installWithBoundary(base, base, 40, 12);
    try {
      const before = retainedIdentityCounterSnapshot();
      expect(boundary.install(next)).toBeGreaterThan(0);
      const after = retainedIdentityCounterSnapshot();
      expect(after.derivation_fast_path_calls - before.derivation_fast_path_calls).toBe(1);
      expect(after.direct_materializer_calls - before.direct_materializer_calls).toBe(1);
      expect(after.bridge_children_visited - before.bridge_children_visited).toBe(0);
      expect(host.screenRows()).toEqual(renderOracle(next, 40, 12));
    } finally {
      boundary.close();
      host.dispose();
    }
  });

  test("§34/§36: wide Grid cell replacement is sequence-backed, not row-copying", () => {
    if (!canRun) return;
    const baseCells = Array.from({ length: 2_000 }, (_, index) => ({ view: View.spacer((index % 3) + 1) }));
    const base = View.grid({ columns: [{ kind: "content" }], rows: [{ track: { kind: "content" }, cells: baseCells }] });
    resetPersistentSeqCounters();
    const next = View.gridSetCellForTransport(base, 0, 1_000, View.spacer(9));
    const seq = { ...persistentSeqCounters };
    expect(seq.branches_cloned).toBeLessThanOrEqual(8);
    expect(seq.items_iterated).toBeLessThanOrEqual(64);
    const { boundary, host } = installWithBoundary(base, base, 60, 20);
    try {
      const before = retainedIdentityCounterSnapshot();
      expect(boundary.install(next)).toBeGreaterThan(0);
      const after = retainedIdentityCounterSnapshot();
      expect(after.derivation_fast_path_calls - before.derivation_fast_path_calls).toBe(1);
      expect(after.direct_materializer_calls - before.direct_materializer_calls).toBe(1);
      expect(after.bridge_children_visited - before.bridge_children_visited).toBe(0);
      expect(host.screenRows()).toEqual(renderOracle(next, 60, 20));
    } finally {
      boundary.close();
      host.dispose();
    }
  });
  test("§36: grid construction remains correct for a larger cell set", () => {
    if (!canRun) return;
    const cells = Array.from({ length: 400 }, (_, index) => ({ view: View.spacer((index % 3) + 1) }));
    const grid = View.grid({
      columns: [{ kind: "content" }],
      rows: [{ track: { kind: "content" }, cells }],
    });
    const host = new Host!(60, 20, true);
    try {
      const boundary = new RetainedRootBoundary(session!, () => host.tuiViewAbiHostPointer() as never);
      expect(boundary.install(grid)).toBeGreaterThan(0);
      expect(host.screenRows().length).toBe(20);
      boundary.close();
    } finally {
      host.dispose();
    }
  });

  test("§50: over-cap grid construction falls back without replacing the old root", () => {
    if (!canRun) return;
    const old = View.spacer(2);
    const host = new Host!(40, 8, true);
    host.render(nodeForBridge(old));
    const boundary = new RetainedRootBoundary(session!, () => host.tuiViewAbiHostPointer() as never);
    try {
      expect(boundary.adopt(old)).toBe(true);
      // 22,000 cells require >65,536 words (header + 3 words/cell), so the
      // retained grid scratch cap refuses before FFI and keeps the old root.
      const cells = Array.from({ length: 22_000 }, () => ({ view: View.spacer(1) }));
      const huge = View.grid({ columns: [{ kind: "content" }], rows: [{ track: { kind: "content" }, cells }] });
      expect(boundary.install(huge)).toBeUndefined();
      expect(boundary.renderExact(old).status).toBe("ok");
    } finally {
      boundary.close();
      host.dispose();
    }
  });
});
