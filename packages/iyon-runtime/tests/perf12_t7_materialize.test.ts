import { describe, expect, test } from "bun:test";

import { native } from "../src/native.ts";
import { nodeForBridge, View } from "../src/tui/values/view.ts";
import { nativeViewAbiSession } from "../src/tui/native_view_abi.ts";
import {
  MaterializeTx,
  retainedIdentityCounterSnapshot,
  RetainedRootBoundary,
  ensureNative,
} from "../src/tui/retained_dag.ts";

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

/** Column spine with `total - 1` text leaves: exactly `total` nodes. */
function buildColumnTree(total: number): View {
  return View.vertical((builder) => {
    for (let index = 0; index < total - 1; index += 1) builder.child(View.text("x"));
  });
}

/**
 * PERF-12 T7: common-node direct materializers. Fixed-size kinds (spacer,
 * row/column arities 0..=4) materialize through monomorphic generated FFI,
 * children first (§22); a stable child cuts off before payload access (§21);
 * the §23 semantic-cache-first rule short-circuits known NodeIds natively;
 * §50 budgets route oversized work to the complete fallback.
 */
describe("PERF-12 T7 common-node direct materializers", () => {
  test("§32: fixed arities 0..=4 materialize and render like the Direct oracle", () => {
    if (!canRun) return;
    const s = session!;
    const oracleHost = new Host(40, 10, true);
    const retainedHost = new Host(40, 10, true);
    try {
      for (let arity = 0; arity <= 4; arity += 1) {
        // Mixed layout-child variants exercise every track-word encoding.
        const build = (horizontal: boolean): View => {
          // Spacer leaves: text payload materializers land in T11, so T7
          // trees compose materializable kinds only. Track variety comes
          // from the layout-child kinds below.
          const leaves = Array.from({ length: arity }, () => View.spacer(1));
          if (horizontal) {
            return View.horizontal((builder) => {
              builder.gap(1);
              for (const leaf of leaves) builder.child(leaf);
            });
          }
          return View.vertical(leaves);
        };

        for (const horizontal of [true, false]) {
          const view = build(horizontal);
          oracleHost.render(nodeForBridge(view));
          const boundary = new RetainedRootBoundary(s, () => retainedHost.tuiViewAbiHostPointer() as never);
          expect(boundary.install(view)).toBeGreaterThan(0);
          expect(retainedHost.screenRows()).toEqual(oracleHost.screenRows());
          boundary.close();
          retainedHost.render(nodeForBridge(View.spacer(1)));
        }
      }
    } finally {
      oracleHost.dispose();
      retainedHost.dispose();
    }
  });

  test("§22/§51: children are materialized once per distinct semantic node", () => {
    if (!canRun) return;
    const s = session!;
    // The same spacer View instance is referenced from two parents: the tx
    // must materialize it exactly once (transaction-local identity).
    const shared = View.spacer(1);
    const root = View.vertical([
      View.horizontal([shared, shared]),
      shared,
      View.spacer(2),
    ]);
    const tx = new MaterializeTx(s.symbols, s.runtime, s.abi.generation, 0);
    const before = retainedIdentityCounterSnapshot();
    const reference = ensureNative(nodeForBridge(root), tx);
    const after = retainedIdentityCounterSnapshot();
    expect(reference).toBeGreaterThan(0);
    // Distinct new nodes: root column + row + spacer(2) + ONE shared spacer.
    expect(after.direct_materializer_calls - before.direct_materializer_calls).toBe(4);
    // The shared spacer was inspected once, not three times.
    expect(after.bridge_semantic_nodes_inspected - before.bridge_semantic_nodes_inspected).toBe(4);
    tx.releaseAll();
  });

  test("SHARED_PATH: stable subtree cuts off before payload access", () => {
    if (!canRun) return;
    const s = session!;
    const host = new Host(60, 16, true);
    try {
      // Previous generation: changed branch A + stable 200-node subtree S.
      const stable = buildColumnTree(200);
      const previous = View.vertical([View.spacer(1), stable]);
      host.render(nodeForBridge(previous));
      const boundary = new RetainedRootBoundary(s, () => host.tuiViewAbiHostPointer() as never);
      expect(boundary.adopt(previous)).toBe(true);

      // Next generation: only branch A is rebuilt; S keeps its identity.
      const next = View.vertical([View.spacer(3), stable]);
      const before = retainedIdentityCounterSnapshot();
      const rootRef = boundary.install(next);
      const after = retainedIdentityCounterSnapshot();

      expect(rootRef).toBeGreaterThan(0);
      // New frontier: new root column + new spacer leaf = 2 materializations.
      expect(after.direct_materializer_calls - before.direct_materializer_calls).toBe(2);
      // The stable subtree resolved by identity alone - one ceiling-gated
      // NodeId->NativeRef promotion at the boundary (cold-sidecar gap, §94)
      // - with zero payload inspection of S or any of its 200 descendants
      // (§21/§51).
      expect(after.node_id_ref_promotion_attempts - before.node_id_ref_promotion_attempts).toBe(1);
      expect(after.node_id_ref_promotion_hits - before.node_id_ref_promotion_hits).toBe(1);
      expect(after.bridge_hint_hits - before.bridge_hint_hits).toBe(0);
      // Children visited: only the new root's two layout slots, never S's.
      expect(after.bridge_children_visited - before.bridge_children_visited).toBe(2);
      expect(after.host_mutations - before.host_mutations).toBe(1);

      // Render parity with a fresh Direct decode of the same tree.
      const oracle = new Host(60, 16, true);
      try {
        oracle.render(nodeForBridge(next));
        expect(host.screenRows()).toEqual(oracle.screenRows());
      } finally {
        oracle.dispose();
      }
      boundary.close();
    } finally {
      host.dispose();
    }
  });

  test("§23: native constructors consult the semantic cache before building", () => {
    if (!canRun) return;
    const s = session!;
    const view = View.vertical([View.spacer(1)]);
    const tx = new MaterializeTx(s.symbols, s.runtime, s.abi.generation, 0);
    const reference = ensureNative(nodeForBridge(view), tx);
    // The tx keeps its lease for the whole probe: the semantic node stays
    // live while we verify the cache-first consult behavior.
    const node = nodeForBridge(view);

    // Direct constructor call (raw symbol: runtime, low, high, gap,
    // track0, child0, track1, child1) with both child refs stale: the
    // cache-first consult must return the live ref without resolving them.
    const high = Math.floor(node.id / 0x1_0000_0000);
    const viaCtor = s.symbols.viewColumnCreate2(
      s.runtime,
      node.id >>> 0,
      high,
      0,
      0,
      0x7fff_fe00,
      0,
      0x7fff_fe00,
    );
    expect(viaCtor).toBe(reference);
    // Drain every acquisition (constructor consult + tx temp lease).
    const drain = Uint32Array.of(viaCtor);
    s.symbols.viewReleaseMany(s.runtime, drain, drain, 1);
    tx.releaseAll();
  });

  test("§50: exceeding the retained work budget falls back and keeps the old root", () => {
    if (!canRun) return;
    const s = session!;
    const host = new Host(40, 8, true);
    try {
      const old = View.spacer(2);
      host.render(nodeForBridge(old));
      const boundary = new RetainedRootBoundary(s, () => host.tuiViewAbiHostPointer() as never);
      expect(boundary.adopt(old)).toBe(true);
      const leasedBefore = memoryLeasedSlots();

      // Nested columns of 4 children each: >512 new nodes within depth 256.
      function wide(totalDepth: number): View {
        let view = View.text("leaf");
        for (let depth = 0; depth < totalDepth; depth += 1) {
          view = View.vertical([view, view === undefined ? View.text("") : cloneChild(depth), extraChild(), extraChild(), extraChild()]);
        }
        return view;
      }
      const cloneChild = (depth: number): View => View.text(`s${depth}`);
      const extraChild = (): View => View.spacer(1);
      const huge = wide(300);

      expect(boundary.install(huge)).toBeUndefined();
      // Every temporary lease drained: lease count back to the old root only.
      expect(memoryLeasedSlots()).toBe(leasedBefore);
      expect(boundary.renderExact(old).status).toBe("ok");
      boundary.close();
    } finally {
      host.dispose();
    }

    function memoryLeasedSlots(): number | undefined {
      return (
        native as {
          tuiViewRuntimeMemorySnapshot?: (countLive?: boolean) => { leased_slots: number };
        }
      ).tuiViewRuntimeMemorySnapshot?.(false)?.leased_slots;
    }
  });

  test("arity beyond the fixed-arity family uses the T8 borrowed-buffer lane", () => {
    if (!canRun) return;
    const s = session!;
    const host = new Host(40, 60, true);
    try {
      const old = View.spacer(2);
      host.render(nodeForBridge(old));
      const boundary = new RetainedRootBoundary(s, () => host.tuiViewAbiHostPointer() as never);
      expect(boundary.adopt(old)).toBe(true);

      // Since T8, arities above the fixed family transport through the
      // reusable borrowed scratch and view_axis_create_buffer.
      const seven = View.vertical(Array.from({ length: 7 }, (_, index) => View.spacer(index + 1)));
      const before = retainedIdentityCounterSnapshot();
      expect(boundary.install(seven)).toBeGreaterThan(0);
      const after = retainedIdentityCounterSnapshot();
      // 7 child refs + 7 track words = 14 transported words (§90 visibility).
      expect(after.ref_words_written - before.ref_words_written).toBe(14);
      boundary.close();
    } finally {
      host.dispose();
    }
  });
});
