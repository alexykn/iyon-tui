import { describe, expect, test } from "bun:test";

import { native } from "../src/native.ts";
import { nativeViewAbiSession } from "../src/tui/native_view_abi.ts";
import { NativeAbiStatusError, viewColumnCreate2, viewReleaseMany, viewSpacerCreate } from "../src/tui/generated/view_calls.ts";
import { nodeForBridge, View } from "../src/tui/values/view.ts";
import {
  forceBridgeNativeHintForTests,
  retainedIdentityCounterSnapshot,
  RetainedRootBoundary,
} from "../src/tui/retained_dag.ts";

interface Host {
  render(view: object): void;
  screenRows(): string[];
  tuiViewAbiHostPointer(): number;
  dispose(): void;
}

const Host = native.NativeTuiHost as unknown as
  | (new (width: number, height: number, headless: boolean) => Host)
  | undefined;
const session = nativeViewAbiSession();
const canRun = Host !== undefined && session !== undefined;

const memorySnapshot = () =>
  (native as { tuiViewRuntimeMemorySnapshot?: (countLive?: boolean) => { leased_slots: number } })
    .tuiViewRuntimeMemorySnapshot?.(true);

function pair(view: View): readonly [number, number] {
  const id = nodeForBridge(view).id;
  return [id >>> 0, Math.floor(id / 0x1_0000_0000)];
}

function oracle(view: View, width = 48, height = 12): string[] {
  const host = new Host!(width, height, true);
  try {
    host.render(nodeForBridge(view));
    return host.screenRows();
  } finally {
    host.dispose();
  }
}

describe("PERF-12 T12 transaction integrity", () => {
  test("§43: materializes a shared child once across multiple changed branches", () => {
    if (!canRun) return;
    const shared = View.spacer(2);
    const next = View.horizontal([shared, shared]);
    const host = new Host!(48, 12, true);
    const boundary = new RetainedRootBoundary(session!, () => host.tuiViewAbiHostPointer() as never);
    try {
      const before = retainedIdentityCounterSnapshot();
      expect(boundary.install(next)).toBeGreaterThan(0);
      const after = retainedIdentityCounterSnapshot();
      // One row plus one shared spacer: the second branch is a transaction
      // local identity hit, not a second constructor/publication.
      expect(after.direct_materializer_calls - before.direct_materializer_calls).toBe(2);
      expect(host.screenRows()).toEqual(oracle(next));
    } finally {
      boundary.close();
      host.dispose();
    }
  });

  test("§74/§47: native child status identifies the stale child ordinal", () => {
    if (!canRun) return;
    const child = View.spacer(1);
    const second = View.spacer(2);
    const next = View.horizontal([child, second]);
    const childRef = viewSpacerCreate(session!.symbols, session!.runtime, ...pair(child), 1);
    try {
      forceBridgeNativeHintForTests(nodeForBridge(child), {
        generation: session!.abi.generation,
        nativeRef: 0x7fff_fe00,
      });
      expect(() => viewColumnCreate2(
        session!.symbols,
        session!.runtime,
        ...pair(next),
        0,
        0,
        0x7fff_fe00,
        0,
        childRef,
      )).toThrow(NativeAbiStatusError);
      try {
        viewColumnCreate2(
          session!.symbols,
          session!.runtime,
          ...pair(next),
          0,
          0,
          0x7fff_fe00,
          0,
          childRef,
        );
      } catch (error) {
        expect(error).toBeInstanceOf(NativeAbiStatusError);
        const detail = (error as NativeAbiStatusError).detail;
        expect(detail & 0xc000_0000).toBe(0x4000_0000);
        expect(detail & 0x3fff_ffff).toBe(0);
      }
    } finally {
      viewReleaseMany(session!.symbols, session!.runtime, Uint32Array.of(childRef), 1);
    }
  });

  test("§47: retries one stale child and keeps the complete transaction atomic", () => {
    if (!canRun) return;
    const first = View.spacer(1);
    const second = View.spacer(2);
    const base = View.horizontal([first, second]);
    const host = new Host!(48, 12, true);
    host.render(nodeForBridge(base));
    const boundary = new RetainedRootBoundary(session!, () => host.tuiViewAbiHostPointer() as never);
    try {
      expect(boundary.adopt(base)).toBe(true);
      forceBridgeNativeHintForTests(nodeForBridge(first), {
        generation: session!.abi.generation,
        nativeRef: 0x7fff_fd00,
      });
      const next = View.horizontal([first, View.spacer(3)]);
      const before = retainedIdentityCounterSnapshot();
      expect(boundary.install(next)).toBeGreaterThan(0);
      const after = retainedIdentityCounterSnapshot();
      expect(after.stale_ref_retries - before.stale_ref_retries).toBe(1);
      expect(host.screenRows()).toEqual(oracle(next));
    } finally {
      boundary.close();
      host.dispose();
    }
  });

  test("§47: retries a stale derivation base exactly once", () => {
    if (!canRun) return;
    const base = View.text("base recovery");
    const host = new Host!(48, 12, true);
    host.render(nodeForBridge(base));
    const boundary = new RetainedRootBoundary(session!, () => host.tuiViewAbiHostPointer() as never);
    try {
      expect(boundary.adopt(base)).toBe(true);
      forceBridgeNativeHintForTests(nodeForBridge(base), {
        generation: session!.abi.generation,
        nativeRef: 0x7fff_f900,
      });
      const before = retainedIdentityCounterSnapshot();
      const next = base.wrap("grapheme");
      expect(boundary.install(next)).toBeGreaterThan(0);
      const after = retainedIdentityCounterSnapshot();
      expect(after.stale_ref_retries - before.stale_ref_retries).toBe(1);
      expect(host.screenRows()).toEqual(oracle(next));
    } finally {
      boundary.close();
      host.dispose();
    }
  });

  test("§47/§73: dormant stale child uses the authoritative Direct recovery helper", () => {
    if (!canRun) return;
    const text = View.text("dormant recovery");
    const oldRoot = View.horizontal([text]);
    const oldHost = new Host!(48, 12, true);
    const oldBoundary = new RetainedRootBoundary(session!, () => oldHost.tuiViewAbiHostPointer() as never);
    try {
      oldHost.render(nodeForBridge(oldRoot));
      expect(oldBoundary.adopt(oldRoot)).toBe(true);
    } finally {
      oldBoundary.close();
      oldHost.dispose();
    }
    Bun.gc(true);
    native.tuiViewAbiMaintain?.(true);
    forceBridgeNativeHintForTests(nodeForBridge(text), {
      generation: session!.abi.generation,
      nativeRef: 0x7fff_fc00,
    });

    const next = View.horizontal([text, View.spacer(4)]);
    const host = new Host!(48, 12, true);
    const boundary = new RetainedRootBoundary(session!, () => host.tuiViewAbiHostPointer() as never);
    try {
      const before = retainedIdentityCounterSnapshot();
      expect(boundary.install(next)).toBeGreaterThan(0);
      const after = retainedIdentityCounterSnapshot();
      expect(after.stale_ref_retries - before.stale_ref_retries).toBe(1);
      expect(host.screenRows()).toEqual(oracle(next));
    } finally {
      boundary.close();
      host.dispose();
    }
  });

  test("§45/§118: child failure drains temporary leases and leaves the old host root", () => {
    if (!canRun) return;
    const old = View.spacer(2);
    const bad = View.horizontal([View.spacer(3), View.text("unsupported retained child")]);
    const host = new Host!(48, 12, true);
    host.render(nodeForBridge(old));
    const boundary = new RetainedRootBoundary(session!, () => host.tuiViewAbiHostPointer() as never);
    try {
      expect(boundary.adopt(old)).toBe(true);
      const beforeRows = host.screenRows();
      const leasedBefore = memorySnapshot()?.leased_slots;
      expect(boundary.install(bad)).toBeUndefined();
      expect(host.screenRows()).toEqual(beforeRows);
      expect(memorySnapshot()?.leased_slots).toBe(leasedBefore);
      expect(boundary.renderExact(old).status).toBe("ok");
      expect(boundary.install(View.horizontal([View.spacer(3), View.spacer(4)]))).toBeGreaterThan(0);
    } finally {
      boundary.close();
      host.dispose();
    }
  });

  test("§45/§118: a second stale child does not trigger an unbounded retry", () => {
    if (!canRun) return;
    const first = View.spacer(1);
    const second = View.spacer(2);
    const old = View.horizontal([first, second]);
    const next = View.horizontal([first, second]);
    const host = new Host!(48, 12, true);
    host.render(nodeForBridge(old));
    const boundary = new RetainedRootBoundary(session!, () => host.tuiViewAbiHostPointer() as never);
    try {
      expect(boundary.adopt(old)).toBe(true);
      forceBridgeNativeHintForTests(nodeForBridge(first), {
        generation: session!.abi.generation,
        nativeRef: 0x7fff_fb00,
      });
      forceBridgeNativeHintForTests(nodeForBridge(second), {
        generation: session!.abi.generation,
        nativeRef: 0x7fff_fa00,
      });
      const before = retainedIdentityCounterSnapshot();
      const beforeRows = host.screenRows();
      expect(boundary.install(next)).toBeUndefined();
      const after = retainedIdentityCounterSnapshot();
      expect(after.stale_ref_retries - before.stale_ref_retries).toBe(1);
      expect(host.screenRows()).toEqual(beforeRows);
      expect(boundary.renderExact(old).status).toBe("ok");
    } finally {
      boundary.close();
      host.dispose();
    }
  });

  test("§45/§118: host failure does not swap the boundary lease", () => {
    if (!canRun) return;
    const old = View.spacer(2);
    const host = new Host!(48, 12, true);
    host.render(nodeForBridge(old));
    const boundary = new RetainedRootBoundary(session!, () => host.tuiViewAbiHostPointer() as never);
    try {
      expect(boundary.adopt(old)).toBe(true);
      const leasedBefore = memorySnapshot()?.leased_slots;
      host.dispose();
      expect(boundary.install(View.spacer(7))).toBeUndefined();
      expect(memorySnapshot()?.leased_slots).toBe(leasedBefore);
    } finally {
      boundary.close();
    }
  });
});
