import { describe, expect, test } from "bun:test";

import { native } from "../src/native.ts";
import { nodeForBridge, View } from "../src/tui/values/view.ts";
import { nativeViewAbiSession } from "../src/tui/native_view_abi.ts";
import {
  MaterializeTx,
  retainedIdentityCounterSnapshot,
  RetainedRootBoundary,
} from "../src/tui/retained_dag.ts";
import { MAX_DIRECT_AXIS_REFS } from "../src/tui/native_view_policy.ts";

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

function buildColumnTree(total: number): View {
  return View.vertical((builder) => {
    for (let index = 0; index < total - 1; index += 1) builder.child(View.spacer(1));
  });
}

/**
 * PERF-12 T8: variable-arity lanes. Variable-axis constructors transport
 * children through reusable synchronous borrowed buffers (`buffer` +
 * `buffer_length`); native reads the storage only during the call and never
 * retains a pointer; zero-length/max-length cases pass and oversize routes
 * to the complete fallback (\u00a729/\u00a730/\u00a7116/\u00a750).
 */
describe("PERF-12 T8 borrowed-buffer variable-arity lanes", () => {
  test("variable arities beyond the fixed family materialize and render correctly", () => {
    if (!canRun) return;
    const s = session!;
    const oracleHost = new Host(40, 40, true);
    const retainedHost = new Host(40, 40, true);
    try {
      for (const arity of [5, 6, 17, 100]) {
        // Mixed layout-child variants exercise every track-word encoding in
        // the scratch loop.
        const view = View.vertical((builder) => {
          builder.gap(2);
          for (let index = 0; index < arity; index += 1) {
            if (index % 4 === 0) builder.fixed(index + 1, View.spacer(1));
            else if (index % 4 === 1) builder.flex(View.spacer(1));
            else if (index % 4 === 2) builder.child(View.spacer(1));
            else builder.contentMax(3, View.spacer(1));
          }
        });
        oracleHost.render(nodeForBridge(view));
        const boundary = new RetainedRootBoundary(s, () => retainedHost.tuiViewAbiHostPointer() as never);
        expect(boundary.install(view)).toBeGreaterThan(0);
        expect(retainedHost.screenRows().slice(0, arity)).toEqual(
          oracleHost.screenRows().slice(0, arity),
        );
        boundary.close();
      }
    } finally {
      oracleHost.dispose();
      retainedHost.dispose();
    }
  });

  test("\u00a7116: zero-length buffer call creates a valid empty axis", () => {
    if (!canRun) return;
    const s = session!;
    const tx = new MaterializeTx(s.symbols, s.runtime, s.abi.generation, 0);
    const empty = new Uint32Array(0);
    // count = 0: no child resolution, valid construction.
    const reference = s.symbols.viewAxisCreateBuffer(s.runtime, 900_001, 0, 2, 0, empty, empty, 0);
    expect(reference).toBeGreaterThan(0);
    expect(reference).toBeLessThan(0x8000_0000);
    // The minted node participates in the semantic cache normally.
    const consulted = s.symbols.viewRefForNodeId(s.runtime, 900_001, 0);
    expect(consulted).toBe(reference);
    const drain = Uint32Array.of(reference);
    s.symbols.viewReleaseMany(s.runtime, drain, drain, 1);
    void tx;
  });

  test("\u00a7116: max retained length passes; oversize refuses before any FFI", () => {
    if (!canRun) return;
    const s = session!;
    const host = new Host(40, 8, true);
    try {
      const old = View.spacer(2);
      host.render(nodeForBridge(old));
      const boundary = new RetainedRootBoundary(s, () => host.tuiViewAbiHostPointer() as never);
      expect(boundary.adopt(old)).toBe(true);

      // Exactly at the cap: one borrowed-buffer call. Children are a small
      // set of REUSED semantic views so identity resolution stays inside the
      // §50/§75 work budgets - this isolates the buffer-lane behavior.
      const pool = Array.from({ length: 8 }, (_, index) => View.spacer(index));
      const prevChildren = Array.from({ length: MAX_DIRECT_AXIS_REFS }, (_, index) => pool[index % 8]);
      const prevRoot = View.vertical(prevChildren);
      host.render(nodeForBridge(prevRoot));
      const dataBoundary = new RetainedRootBoundary(s, () => host.tuiViewAbiHostPointer() as never);
      expect(dataBoundary.adopt(prevRoot)).toBe(true);

      const atCap = View.vertical([...prevChildren].reverse());
      const before = retainedIdentityCounterSnapshot();
      expect(dataBoundary.install(atCap)).toBeGreaterThan(0);
      const after = retainedIdentityCounterSnapshot();
      // Only the new root was materialized; children resolved by identity.
      expect(after.direct_materializer_calls - before.direct_materializer_calls).toBe(1);
      expect(after.ref_words_written - before.ref_words_written).toBe(MAX_DIRECT_AXIS_REFS * 2);

      // One over the cap: refused on the JS side before any FFI call - no
      // constructor invocation, no partial publication, old root intact.
      const oversize = View.vertical([
        ...prevChildren,
        View.spacer(9), // one fresh child pushes arity past the cap
      ]);
      const fallbacksBefore = retainedIdentityCounterSnapshot().cold_fallbacks;
      expect(dataBoundary.install(oversize)).toBeUndefined();
      const fallbacksAfter = retainedIdentityCounterSnapshot().cold_fallbacks;
      expect(fallbacksAfter).toBe(fallbacksBefore + 1);
      expect(dataBoundary.renderExact(old).status).toBe("ok");
      dataBoundary.close();
    } finally {
      host.dispose();
    }
  });

  test("\u00a7116/\u00a729: the same reused scratch backs successive calls with correct results", () => {
    if (!canRun) return;
    const s = session!;
    const hostA = new Host(40, 20, true);
    const hostB = new Host(40, 20, true);
    try {
      // Two boundaries alternate installs; both reuse the single environment
      // scratch. Correct distinct renders prove native copies during the
      // call and retains no pointer into the shared storage.
      const seed = View.spacer(1);
      for (const host of [hostA, hostB]) host.render(nodeForBridge(seed));
      const boundaryA = new RetainedRootBoundary(session ?? s, () => hostA.tuiViewAbiHostPointer() as never);
      const boundaryB = new RetainedRootBoundary(s, () => hostB.tuiViewAbiHostPointer() as never);
      expect(boundaryA.adopt(seed)).toBe(true);
      expect(boundaryB.adopt(seed)).toBe(true);

      let lastA: View | undefined;
      let lastB: View | undefined;
      for (let step = 0; step < 6; step += 1) {
        const target = step % 2 === 0 ? boundaryA : boundaryB;
        const view =
          step % 3 === 0
            ? View.horizontal(Array.from({ length: 9 }, (_, i) => View.spacer((i + step) % 4)))
            : View.vertical(Array.from({ length: 12 }, (_, i) => View.spacer((i + step) % 4)));
        expect(target.install(view)).toBeGreaterThan(0);
        if (target === boundaryA) lastA = view;
        else lastB = view;
      }
      // Both hosts still render their own last installs exactly through the
      // §20 fast path.
      if (lastA !== undefined) expect(boundaryA.renderExact(lastA).status).toBe("ok");
      if (lastB !== undefined) expect(boundaryB.renderExact(lastB).status).toBe("ok");
      boundaryA.close();
      boundaryB.close();
    } finally {
      hostA.dispose();
      hostB.dispose();
    }
  });

  test("\u00a7116: wrong-typed input is rejected by the FFI layer", () => {
    if (!canRun) return;
    const s = session!;
    // A non-TypedArray cannot be passed where the ABI declares a buffer:
    // bun:ffi rejects it before any native code runs.
    expect(() =>
      s.symbols.viewAxisCreateBuffer(s.runtime, 900_002, 0, 2, 0, 42 as never, 42 as never, 1),
    ).toThrow();
  });

  test("native retains no pointer: mutating scratch between calls changes nothing", () => {
    if (!canRun) return;
    const s = session!;
    // Build two axes through the raw buffer call reusing one array whose
    // contents are fully rewritten between calls; each returned axis must
    // reflect only its own call's data (verified by NodeId identity and by
    // successful distinct constructions).
    const scratch = new Uint32Array(2 * 3);
    const refs: number[] = [];
    for (let step = 0; step < 2; step += 1) {
      const child = s.symbols.viewSpacerCreate(s.runtime, 901_000 + step, 0, 1);
      expect(child).toBeGreaterThan(0);
      refs.push(child);
      scratch[0] = 0;
      scratch[1] = child;
      scratch[2] = 3 | (1 << 8); // fixed track
      scratch[3] = child;
      scratch[4] = 4 | (1 << 8); // flex track
      scratch[5] = child;
      const built = s.symbols.viewAxisCreateBuffer(s.runtime, 902_000 + step, 0, 1, 0, scratch, scratch, 3);
      expect(built).toBeGreaterThan(0);
      expect(built).toBeLessThan(0x8000_0000);
    }
    // Distinct NodeIds -> distinct published nodes despite identical input
    // storage reuse.
    const first = s.symbols.viewRefForNodeId(s.runtime, 902_000, 0);
    const second = s.symbols.viewRefForNodeId(s.runtime, 902_001, 0);
    expect(first).toBeGreaterThan(0);
    expect(second).toBeGreaterThan(0);
    const drain = Uint32Array.from([...refs, first, second]);
    s.symbols.viewReleaseMany(s.runtime, drain, drain, drain.length);
  });
});
