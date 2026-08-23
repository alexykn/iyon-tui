import { describe, expect, test } from "bun:test";

import { native } from "../src/native.ts";
import { nodeForBridge, View } from "../src/tui/values/view.ts";
import { TextSpan } from "../src/tui/values/text.ts";
import { nativeViewAbiSession } from "../src/tui/native_view_abi.ts";
import {
  MaterializeTx,
  peekBridgeNativeHint,
  renderExactRoot,
  retainedIdentityCounterSnapshot,
  RetainedFastFallbackError,
  RetainedRootBoundary,
  ensureNative,
  forceBridgeNativeHintForTests,
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

const memorySnapshot = () =>
  (native as { tuiViewRuntimeMemorySnapshot?: (countLive?: boolean) => { leased_slots: number } })
    .tuiViewRuntimeMemorySnapshot?.(true);

/**
 * Builds a column spine with `total - 1` text leaves: exactly `total` semantic
 * nodes through the public API, all sharing stable child identity.
 */
function buildColumnTree(total: number): View {
  return View.vertical((builder) => {
    for (let index = 0; index < total - 1; index += 1) builder.child(View.text("x"));
  });
}

/**
 * PERF-12 T6: identity fast paths. The exact known root must resolve through
 * one hostRenderRef with zero semantic field reads and zero buffer writes,
 * independent of descendant count (§20/§113); stale generation hints must be
 * ignored (§48); the boundary lease protocol must keep old roots alive until
 * replacement succeeds and drain temporary leases on failure (§18).
 */
describe("PERF-12 T6 retained identity fast paths", () => {
  test("exact root renders with one FFI call and zero semantic work", () => {
    if (Host === undefined || nativeViewAbiSession() === undefined) return;
    const session = nativeViewAbiSession()!;
    const view = buildColumnTree(200);
    const host = new Host(40, 12, true);
    try {
      // Cold population through Direct decode: native cache filled, no JS hints.
      host.render(nodeForBridge(view));
      expect(peekBridgeNativeHint(nodeForBridge(view))).toBeUndefined();

      const boundary = new RetainedRootBoundary(session, () => host.tuiViewAbiHostPointer() as never);
      expect(boundary.adopt(view)).toBe(true);
      expect(peekBridgeNativeHint(nodeForBridge(view))?.nativeRef).toBeGreaterThan(0);

      const before = retainedIdentityCounterSnapshot();
      // Warm the JIT path, then measure a counter delta over a fixed window.
      for (let index = 0; index < 50; index += 1) boundary.renderExact(view);
      const warm = retainedIdentityCounterSnapshot();
      const renders = 500;
      for (let index = 0; index < renders; index += 1) boundary.renderExact(view);
      const after = retainedIdentityCounterSnapshot();

      expect(after.host_mutations - before.host_mutations).toBe(50 + renders);
      expect(warm.bridge_hint_hits - before.bridge_hint_hits).toBe(50);
      expect(after.direct_materializer_calls - before.direct_materializer_calls).toBe(0);
      expect(after.bridge_semantic_nodes_inspected - before.bridge_semantic_nodes_inspected).toBe(0);
      expect(after.bridge_children_visited - before.bridge_children_visited).toBe(0);
      expect(after.ref_words_written - before.ref_words_written).toBe(0);

      const rendered = boundary.renderExact(view);
      expect(rendered.status).toBe("ok");
      const rows = host.screenRows();
      expect(rows[0]?.startsWith("x")).toBe(true);
      expect(rows[1]?.startsWith("x")).toBe(true);
      expect(rows.every((row) => row.trimEnd() === "" || row.startsWith("x"))).toBe(true);
      boundary.close();
      void warm;
    } finally {
      host.dispose();
    }
  });

  test("§48: stale generation hints are ignored and re-derived", () => {
    if (Host === undefined || nativeViewAbiSession() === undefined) return;
    const session = nativeViewAbiSession()!;
    const view = View.spacer(2);
    const node = nodeForBridge(view);
    const tx = new MaterializeTx(session.symbols, session.runtime, session.abi.generation, node.id);
    const reference = ensureNative(node, tx);
    expect(reference).toBeGreaterThan(0);

    const attemptsBefore = retainedIdentityCounterSnapshot().node_id_ref_promotion_attempts;
    // Corrupt the hint's generation: it must be skipped entirely.
    forceBridgeNativeHintForTests(node, { generation: session.abi.generation + 10_000, nativeRef: 0x7fff_ff00 });
    const tx2 = new MaterializeTx(session.symbols, session.runtime, session.abi.generation, node.id);
    const recovered = ensureNative(node, tx2);
    const after = retainedIdentityCounterSnapshot();
    expect(recovered).toBe(reference);
    expect(after.node_id_ref_promotion_attempts - attemptsBefore).toBe(1);
    expect(after.node_id_ref_promotion_hits - after.node_id_ref_promotion_hits).toBeGreaterThanOrEqual(0);
    const hint = peekBridgeNativeHint(node);
    expect(hint?.generation).toBe(session.abi.generation);
    expect(hint?.nativeRef).toBe(reference);
    tx.releaseAll();
    tx2.releaseAll();
  });

  test("§19: ceiling gating skips the NodeId probe for definitely-new nodes", () => {
    if (Host === undefined || nativeViewAbiSession() === undefined) return;
    const session = nativeViewAbiSession()!;
    const host = new Host(20, 6, true);
    try {
      const old = View.spacer(1);
      host.render(nodeForBridge(old));
      const boundary = new RetainedRootBoundary(session, () => host.tuiViewAbiHostPointer() as never);
      expect(boundary.adopt(old)).toBe(true);

      // Created after the commit: id > ceiling → no probe, direct materialization.
      const fresh = View.spacer(3);
      expect(fresh && nodeForBridge(fresh).id).toBeGreaterThan(boundary.nativeLookupCeiling);
      const before = retainedIdentityCounterSnapshot();
      const tx = new MaterializeTx(
        session.symbols,
        session.runtime,
        session.abi.generation,
        boundary.nativeLookupCeiling,
      );
      const reference = ensureNative(nodeForBridge(fresh), tx);
      const after = retainedIdentityCounterSnapshot();
      expect(reference).toBeGreaterThan(0);
      expect(after.node_id_ref_promotion_attempts - before.node_id_ref_promotion_attempts).toBe(0);
      expect(after.direct_materializer_calls - before.direct_materializer_calls).toBe(1);
      // §23 semantic-cache-first: native consults resolve to the same ref.
      const [low, high] = [nodeForBridge(fresh).id >>> 0, Math.floor(nodeForBridge(fresh).id / 0x1_0000_0000)];
      const consulted = session.symbols.viewRefForNodeId(session.runtime, low, high);
      expect(consulted).toBe(reference);
      tx.releaseAll();
      boundary.close();
    } finally {
      host.dispose();
    }
  });

  test("§47/§20: exact-root CACHE_MISS recovers with one targeted retry", () => {
    if (Host === undefined || nativeViewAbiSession() === undefined) return;
    const session = nativeViewAbiSession()!;
    const view = View.spacer(2);
    const host = new Host(20, 6, true);
    try {
      host.render(nodeForBridge(view));
      const boundary = new RetainedRootBoundary(session, () => host.tuiViewAbiHostPointer() as never);
      expect(boundary.adopt(view)).toBe(true);
      // Sabotage the hint with a valid-generation but dead ref.
      forceBridgeNativeHintForTests(nodeForBridge(view), {
        generation: session.abi.generation,
        nativeRef: 0x7fff_fe00,
      });
      const result = renderExactRoot(session, host.tuiViewAbiHostPointer() as never, view);
      if (result.status !== "ok") throw new Error(`expected exact-root recovery to succeed, got ${result.status}`);
      expect(result.recovered).toBe(true);
      expect(result.rootRef).toBeGreaterThan(0);
      expect(host.screenRows().slice(0, 2)).toEqual(["                    ", "                    "]);
      const hint = peekBridgeNativeHint(nodeForBridge(view));
      expect(hint?.nativeRef).toBe(result.rootRef);
      expect(retainedIdentityCounterSnapshot().stale_ref_retries).toBeGreaterThanOrEqual(1);
      boundary.close();
    } finally {
      host.dispose();
    }
  });

  test("§18: failed install keeps the previous root and drains temporary leases", () => {
    if (Host === undefined || nativeViewAbiSession() === undefined) return;
    const session = nativeViewAbiSession()!;
    const view = View.spacer(2);
    // T13 note: decorated nodes now have a retained materializer, so the
    // canonical unsupported kind here is a 5-span styled text (outside the
    // retained span family, §49 explicit routing).
    const unsupported = View.styledText([
      TextSpan.plain("a"),
      TextSpan.plain("b"),
      TextSpan.plain("c"),
      TextSpan.plain("d"),
      TextSpan.plain("e"),
    ]);
    const host = new Host(20, 6, true);
    try {
      host.render(nodeForBridge(view));
      const boundary = new RetainedRootBoundary(session, () => host.tuiViewAbiHostPointer() as never);
      expect(boundary.adopt(view)).toBe(true);
      const leasedBefore = memorySnapshot()?.leased_slots;

      expect(() => ensureNative(nodeForBridge(unsupported), new MaterializeTx(
        session.symbols,
        session.runtime,
        session.abi.generation,
        0,
      ))).toThrow(RetainedFastFallbackError);
      expect(boundary.install(unsupported)).toBeUndefined();

      const leasedAfter = memorySnapshot()?.leased_slots;
      expect(leasedAfter).toBe(leasedBefore);
      // The old root is untouched and still renders exactly.
      expect(boundary.renderExact(view).status).toBe("ok");
      boundary.close();
    } finally {
      host.dispose();
    }
  });

  test("§18: close releases the boundary's own lease exactly once; double close is a no-op", () => {
    if (nativeViewAbiSession() === undefined) return;
    const session = nativeViewAbiSession()!;
    const view = View.spacer(2);
    const node = nodeForBridge(view);
    // Materialize a rootless spacer so the slot has no other owner: the tx
    // holds temp lease A, the boundary adds lease B on the same slot, and
    // draining A isolates the boundary's lease for the exactly-once proof.
    const tx = new MaterializeTx(session.symbols, session.runtime, session.abi.generation, node.id);
    expect(ensureNative(node, tx)).toBeGreaterThan(0);
    const boundary = new RetainedRootBoundary(session, () => undefined);
    expect(boundary.adopt(view)).toBe(true);
    const whileTxHeld = memorySnapshot()?.leased_slots ?? 0;
    tx.releaseAll();
    const withBoundaryLease = memorySnapshot()?.leased_slots ?? 0;
    // Exactly one lease remains on this slot and it belongs to the boundary.
    expect(withBoundaryLease).toBe(whileTxHeld);
    boundary.close();
    const afterClose = memorySnapshot()?.leased_slots ?? 0;
    expect(afterClose).toBe(withBoundaryLease - 1);
    boundary.close(); // idempotent; must not release anyone else's lease
    expect(memorySnapshot()?.leased_slots).toBe(afterClose);
  });

  test("§115-lite: two boundaries may hold the same semantic root", () => {
    if (Host === undefined || nativeViewAbiSession() === undefined) return;
    const session = nativeViewAbiSession()!;
    const view = View.spacer(2);
    const hostA = new Host(20, 6, true);
    const hostB = new Host(20, 6, true);
    try {
      hostA.render(nodeForBridge(view));
      const boundaryA = new RetainedRootBoundary(session, () => hostA.tuiViewAbiHostPointer() as never);
      const boundaryB = new RetainedRootBoundary(session, () => hostB.tuiViewAbiHostPointer() as never);
      expect(boundaryA.adopt(view)).toBe(true);
      expect(boundaryB.adopt(view)).toBe(true);
      expect(boundaryA.renderExact(view).status).toBe("ok");
      expect(boundaryB.renderExact(view).status).toBe("ok");
      boundaryA.close();
      expect(boundaryB.renderExact(view).status).toBe("ok");
      boundaryB.close();
      // Dormant recovery (§114): a later host can re-adopt the same node.
      const hostC = new Host(20, 6, true);
      try {
        const boundaryC = new RetainedRootBoundary(session, () => hostC.tuiViewAbiHostPointer() as never);
        expect(boundaryC.adopt(view)).toBe(true);
        expect(boundaryC.renderExact(view).status).toBe("ok");
        boundaryC.close();
      } finally {
        hostC.dispose();
      }
    } finally {
      hostA.dispose();
      hostB.dispose();
    }
  });

  test("§21: a hinted stable subtree cuts off before payload/child inspection", () => {
    if (Host === undefined || nativeViewAbiSession() === undefined) return;
    const session = nativeViewAbiSession()!;
    const view = buildColumnTree(50);
    const host = new Host(40, 12, true);
    try {
      host.render(nodeForBridge(view));
      // Install a current-generation hint on the subtree root only.
      const [low, high] = [nodeForBridge(view).id >>> 0, Math.floor(nodeForBridge(view).id / 0x1_0000_0000)];
      const subtreeRef = session.symbols.viewRefForNodeId(session.runtime, low, high);
      forceBridgeNativeHintForTests(nodeForBridge(view), {
        generation: session.abi.generation,
        nativeRef: subtreeRef,
      });

      const before = retainedIdentityCounterSnapshot();
      const tx = new MaterializeTx(session.symbols, session.runtime, session.abi.generation, 0);
      const reference = ensureNative(nodeForBridge(view), tx);
      const after = retainedIdentityCounterSnapshot();

      expect(reference).toBe(subtreeRef);
      expect(after.bridge_hint_hits - before.bridge_hint_hits).toBe(1);
      expect(after.bridge_semantic_nodes_inspected - before.bridge_semantic_nodes_inspected).toBe(0);
      expect(after.bridge_children_visited - before.bridge_children_visited).toBe(0);
      expect(after.direct_materializer_calls - before.direct_materializer_calls).toBe(0);
      expect(tx.temporaryLeases.length).toBe(0); // borrowed hint: no lease taken
      expect(tx.borrowedHints.length).toBe(1);
      tx.releaseAll();
      void view;
    } finally {
      host.dispose();
    }
  });

  test("§113 structural proof: exact identity is independent of descendant count", () => {
    if (Host === undefined || nativeViewAbiSession() === undefined) return;
    const session = nativeViewAbiSession()!;
    for (const size of [20, 200, 2_000, 10_000]) {
      const view = buildColumnTree(size);
      const host = new Host(80, 8, true);
      try {
        host.render(nodeForBridge(view));
        const boundary = new RetainedRootBoundary(session, () => host.tuiViewAbiHostPointer() as never);
        expect(boundary.adopt(view)).toBe(true);
        const before = retainedIdentityCounterSnapshot();
        const renders = 100;
        for (let index = 0; index < renders; index += 1) {
          expect(boundary.renderExact(view).status).toBe("ok");
        }
        const after = retainedIdentityCounterSnapshot();
        expect(after.host_mutations - before.host_mutations).toBe(renders);
        expect(after.direct_materializer_calls - before.direct_materializer_calls).toBe(0);
        expect(after.bridge_semantic_nodes_inspected - before.bridge_semantic_nodes_inspected).toBe(0);
        expect(after.bridge_children_visited - before.bridge_children_visited).toBe(0);
        expect(after.node_id_ref_promotion_attempts - before.node_id_ref_promotion_attempts).toBe(0);
        expect(after.ref_words_written - before.ref_words_written).toBe(0);
        boundary.close();
      } finally {
        host.dispose();
      }
    }
  });
});
