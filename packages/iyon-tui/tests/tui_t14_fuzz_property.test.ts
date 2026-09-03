import { describe, expect, test } from "bun:test";
import { nativeViewAbiSession } from "../src/transport/structural/native-view-abi.ts";
import {
  viewReleaseMany,
  viewRenderRef,
  viewTextCreateBuffer,
} from "../src/transport/abi/structural/generated/view_calls.ts";
import { nodeIdPair, View } from "../src/api/view/view.ts";
import { AppHarness } from "../src/testing/index.ts";

/** Corrupt words+bytes framings for the variadic text constructor. */
function malformedFraming(seed: number): { words: Uint32Array; bytes: Uint8Array } {
  switch (seed % 6) {
    case 0:
      // Claims spans but carries no framing at all.
      return { words: new Uint32Array(0), bytes: new Uint8Array(0) };
    case 1:
      // Zero spans.
      return { words: new Uint32Array([0]), bytes: new Uint8Array(0) };
    case 2:
      // Span count disagrees with the framed pairs.
      return { words: new Uint32Array([5, 0, 3]), bytes: new Uint8Array([97, 98, 99]) };
    case 3: {
      // Span byte length overruns the payload.
      const words = new Uint32Array([1, 0, 99]);
      return { words, bytes: new Uint8Array([97]) };
    }
    case 4:
      // Trailing garbage after the framed spans.
      return { words: new Uint32Array([1, 0, 1, 7, 7]), bytes: new Uint8Array([97]) };
    default: {
      // Invalid UTF-8 payload bytes.
      const words = new Uint32Array([1, 0, 2]);
      return { words, bytes: new Uint8Array([0xff, 0xfe]) };
    }
  }
}

describe("PERF-12 T14 malformed-boundary properties", () => {
  test("malformed payloads and invalid refs fail without partially mutating the host", async () => {
    const session = nativeViewAbiSession();
    if (session === undefined) return;
    const tui = await AppHarness.open({ width: 12, height: 4 });
    try {
      tui.render({ body: View.text("stable") });
      const baseline = tui.screenRows();
      // Valid wrap/align codes (3 = noWrap, 1 = start) so every failure below
      // comes from the corrupt framing itself.
      for (let seed = 1; seed <= 100; seed++) {
        const framing = malformedFraming(seed);
        const view = View.text(`node-${seed}`);
        const [low, high] = nodeIdPair(view);
        let accepted = false;
        try {
          viewTextCreateBuffer(
            session.symbols,
            session.runtime,
            low,
            high,
            framing.words,
            framing.words.length,
            framing.bytes,
            framing.bytes.length,
            3,
            1,
          );
          accepted = true;
        } catch {
          // Invalid payloads are expected to fail before host mutation.
        }
        expect(accepted, `malformed framing seed ${seed}`).toBe(false);
        expect(tui.screenRows(), `malformed seed ${seed}`).toEqual(baseline);
        try {
          viewRenderRef(session.symbols, session.runtime, 0xffff_ffff - seed);
        } catch {
          // Invalid NativeRefs are expected to fail before host mutation.
        }
      }
      // Unknown wrap/align codes fail even with well-formed framing.
      const view = View.text("bad-layout-codes");
      const [low, high] = nodeIdPair(view);
      const words = new Uint32Array([1, 0, 1]);
      const bytes = new Uint8Array([120]);
      expect(() =>
        viewTextCreateBuffer(session.symbols, session.runtime, low, high, words, 3, bytes, 1, 99, 1),
      ).toThrow();
      expect(tui.screenRows()).toEqual(baseline);
    } finally {
      tui.close();
    }
  });

  test("a live NodeId is served from the semantic cache without reading the payload", () => {
    const session = nativeViewAbiSession();
    if (session === undefined) return;
    const view = View.text("cache-skips-payload");
    const [low, high] = nodeIdPair(view);
    const words = new Uint32Array([1, 0, 5]);
    const bytes = new Uint8Array([104, 101, 108, 108, 111]);
    const reference = viewTextCreateBuffer(
      session.symbols,
      session.runtime,
      low,
      high,
      words,
      words.length,
      bytes,
      bytes.length,
      1,
      1,
    );
    try {
      expect(reference).toBeGreaterThan(0);
      // The same NodeId with a corrupt payload still resolves: the live
      // entry wins before any payload byte is read.
      const corrupt = new Uint32Array([0]);
      const cached = viewTextCreateBuffer(
        session.symbols,
        session.runtime,
        low,
        high,
        corrupt,
        corrupt.length,
        new Uint8Array(0),
        0,
        1,
        1,
      );
      expect(cached).toBe(reference);
    } finally {
      viewReleaseMany(session.symbols, session.runtime, new Uint32Array([reference]), 1);
    }
  });
});
