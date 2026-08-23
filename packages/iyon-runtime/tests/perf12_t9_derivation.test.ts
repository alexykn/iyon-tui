import { describe, expect, test } from "bun:test";

import { native } from "../src/native.ts";
import { nodeForBridge, View } from "../src/tui/values/view.ts";
import { nativeViewAbiSession } from "../src/tui/native_view_abi.ts";
import {
  retainedIdentityCounterSnapshot,
  RetainedRootBoundary,
} from "../src/tui/retained_dag.ts";
import { peekBridgeDerivation, BRIDGE_VIEW_KIND } from "../src/tui/ir.ts";

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

function leasedSlots(): number | undefined {
  return (
    native as {
      tuiViewRuntimeMemorySnapshot?: (countLive?: boolean) => { leased_slots: number };
    }
  ).tuiViewRuntimeMemorySnapshot?.(false)?.leased_slots;
}

/**
 * PERF-12 T9: retained clone/edit lanes — derivation hints (§27–§28) and
 * text layout mutation (§38). A wrap/align-only text change transports base
 * NativeRef + NodeId + scalars and never resends the payload; a scalar-only
 * decoration patch reuses the base ref through the common patch; a hint miss
 * degrades cleanly to full materialization.
 *
 * Base liveness follows §18: the base sits inside the previous generation's
 * root, whose lease is held for the entire replacement, so its native View
 * stays resolvable while the derivation runs.
 */
describe("PERF-12 T9 derivation hints", () => {
  test("§38: wrap-only text change clones the retained payload, never resends it", () => {
    if (!canRun) return;
    const s = session!;
    const host = new Host(40, 8, true);
    try {
      // Previous generation: the unmodified text lives inside the leased
      // root, so its native View stays resolvable (§18).
      const text = View.text("hello retained world");
      const previous = View.vertical([View.spacer(1), text]);
      host.render(nodeForBridge(previous));
      const boundary = new RetainedRootBoundary(s, () => host.tuiViewAbiHostPointer() as never);
      expect(boundary.adopt(previous)).toBe(true);

      const next = View.vertical([View.spacer(1), text.wrap("grapheme")]);
      const nextRoot = nodeForBridge(next);
      const derivedText =
        nextRoot.kind === BRIDGE_VIEW_KIND.column || nextRoot.kind === BRIDGE_VIEW_KIND.row
          ? nextRoot.children[1]!.child
          : nextRoot;
      // The modifier attached exactly one derivation hint.
      expect(peekBridgeDerivation(derivedText)).toEqual({
        kind: "textLayout",
        base: nodeForBridge(text),
        wrap: 2,
        align: 1,
      });

      const before = retainedIdentityCounterSnapshot();
      expect(boundary.install(next)).toBeGreaterThan(0);
      const after = retainedIdentityCounterSnapshot();

      // The derived text rode the derivation fast path; the new root column
      // and the fresh spacer went through generated materializers.
      expect(after.derivation_fast_path_calls - before.derivation_fast_path_calls).toBe(1);
      expect(after.direct_materializer_calls - before.direct_materializer_calls).toBe(2);
      // No payload bytes were transported anywhere (§90 visibility).
      expect(after.byte_payload_bytes - before.byte_payload_bytes).toBe(0);
      // The base was recovered by one ceiling-gated promotion (no JS hint
      // existed on it after the Direct decode - the §94 cold-sidecar shape).
      expect(after.node_id_ref_promotion_attempts - before.node_id_ref_promotion_attempts).toBe(1);
      expect(after.node_id_ref_promotion_hits - before.node_id_ref_promotion_hits).toBe(1);
      expect(after.cold_fallbacks - before.cold_fallbacks).toBe(0);
      expect(after.host_mutations - before.host_mutations).toBe(1);

      // Render parity with a fresh Direct decode of the same tree.
      const oracle = new Host(40, 8, true);
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

  test("§38: align-only change derives the text node and renders identically", () => {
    if (!canRun) return;
    const s = session!;
    const host = new Host(40, 8, true);
    try {
      const text = View.text("aligned payload");
      const previous = View.vertical([text]);
      host.render(nodeForBridge(previous));
      const boundary = new RetainedRootBoundary(s, () => host.tuiViewAbiHostPointer() as never);
      expect(boundary.adopt(previous)).toBe(true);

      const next = View.vertical([text.textAlign("center")]);
      const before = retainedIdentityCounterSnapshot();
      expect(boundary.install(next)).toBeGreaterThan(0);
      const after = retainedIdentityCounterSnapshot();
      expect(after.derivation_fast_path_calls - before.derivation_fast_path_calls).toBe(1);
      expect(after.byte_payload_bytes - before.byte_payload_bytes).toBe(0);
      expect(after.cold_fallbacks - before.cold_fallbacks).toBe(0);

      const oracle = new Host(40, 8, true);
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

  test("§27/§28: scalar-only decoration patch reuses the shared base ref", () => {
    if (!canRun) return;
    const s = session!;
    const host = new Host(30, 12, true);
    try {
      // The unmodified base child keeps its identity across generations;
      // each generation decorates it with different scalar-only modifiers.
      const shared = View.text("stable payload");
      const previous = View.vertical([shared]);
      host.render(nodeForBridge(previous));
      const boundary = new RetainedRootBoundary(s, () => host.tuiViewAbiHostPointer() as never);
      expect(boundary.adopt(previous)).toBe(true);

      // Generation 1: padding scalars ride the common-scalar patch.
      const v1 = View.vertical([shared.padding(2)]);
      const v1Root = nodeForBridge(v1);
      const paddedNode =
        v1Root.kind === BRIDGE_VIEW_KIND.column || v1Root.kind === BRIDGE_VIEW_KIND.row
          ? v1Root.children[0]!.child
          : v1Root;
      expect(peekBridgeDerivation(paddedNode)).toMatchObject({ kind: "commonScalar", mask: 4 });
      let before = retainedIdentityCounterSnapshot();
      expect(boundary.install(v1)).toBeGreaterThan(0);
      let after = retainedIdentityCounterSnapshot();
      expect(after.derivation_fast_path_calls - before.derivation_fast_path_calls).toBe(1);
      expect(after.direct_materializer_calls - before.direct_materializer_calls).toBe(1);

      // Generation 2: a width rule on the same base rides the same lane.
      const v2 = View.vertical([shared.fitWidth()]);
      before = retainedIdentityCounterSnapshot();
      expect(boundary.install(v2)).toBeGreaterThan(0);
      after = retainedIdentityCounterSnapshot();
      expect(after.derivation_fast_path_calls - before.derivation_fast_path_calls).toBe(1);
      expect(after.byte_payload_bytes - before.byte_payload_bytes).toBe(0);

      // Render parity for the final state.
      const oracle = new Host(30, 12, true);
      try {
        oracle.render(nodeForBridge(v2));
        expect(host.screenRows()).toEqual(oracle.screenRows());
      } finally {
        oracle.dispose();
      }
      boundary.close();
    } finally {
      host.dispose();
    }
  });

  test("mixed decorations stay unhinted and never ride the scalar patch", () => {
    if (!canRun) return;
    const s = session!;
    const host = new Host(30, 10, true);
    try {
      const old = View.spacer(2);
      host.render(nodeForBridge(old));
      const boundary = new RetainedRootBoundary(s, () => host.tuiViewAbiHostPointer() as never);
      expect(boundary.adopt(old)).toBe(true);
      const leasedBefore = leasedSlots();
      const derivationsBefore = retainedIdentityCounterSnapshot().derivation_fast_path_calls;

      // A style-bearing decoration has no exact retained primitive: no hint
      // may be attached (T13 note: the decorated node now materializes
      // through its own constructor — the §27 guarantee under test is that it
      // does NOT ride the common-scalar patch lane).
      const mixed = View.spacer(1).bold();
      expect(peekBridgeDerivation(nodeForBridge(mixed))).toBeUndefined();
      expect(boundary.install(View.vertical([mixed]))).toBeGreaterThan(0);

      // No derivation fast path fired; leases return to exactly one boundary
      // root lease; the previous root still renders exactly.
      expect(retainedIdentityCounterSnapshot().derivation_fast_path_calls).toBe(derivationsBefore);
      // The boundary transferred its root lease to the new root: still exactly
      // one durable boundary lease, and the previous root renders via hint.
      expect(leasedSlots()).toBe(leasedBefore);
      expect(boundary.renderExact(old).status).toBe("ok");
      boundary.close();
    } finally {
      host.dispose();
    }
  });

  test("stale base ref degrades natively and the hint is ignored, not fatal", () => {
    if (!canRun) return;
    const s = session!;
    // Raw-symbol proof: a stale/unresolvable base surfaces an error status
    // (never a crash), which tryDerivation converts into "ignore hint" and
    // materialize from semantic fields (§27/§38 fallback rule).
    const ERROR_BIT = 0x8000_0000;
    const stale = s.symbols.viewTextLayoutPatchRoot(s.runtime, 0x7fff_fe10, 900_000, 0, 2, 1);
    expect(stale >= ERROR_BIT).toBe(true);
    const staleCommon = s.symbols.viewCommonPatchRoot(
      s.runtime,
      0x7fff_fe11,
      900_001,
      0,
      4,
      1,
      0,
      0,
      0,
      0,
      0,
      0,
      0,
      0,
    );
    expect(staleCommon >= ERROR_BIT).toBe(true);
  });
});
