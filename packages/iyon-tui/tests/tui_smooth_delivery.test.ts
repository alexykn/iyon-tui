import { expect, test } from "bun:test";

import { TextFunnel, TextStreamSource, View } from "../src/index.ts";
import { AppHarness } from "../src/testing/index.ts";

const SMOOTH = "PRE-V5-L1 smooth delivery characterization";

function revealedLineCount(rows: readonly string[]): number {
  return rows.filter((row) => row.trimEnd().length > 0).length;
}

test(`${SMOOTH} reveals gradually under the deterministic clock`, async () => {
  const harness = await AppHarness.open({ width: 30, height: 6 });
  const source = TextStreamSource.create({
    retention: { maxBytes: 64 * 1024, overflow: "drop-oldest" },
  });
  try {
    const port = harness.contentPort();
    const connector = port.connect(
      source,
      TextFunnel.plain().smooth({ tickIntervalMs: 50, minUnitsPerSecond: 1, maxUnitsPerSecond: 100 }),
    );
    connector.activate();
    harness.render({ body: View.content(port) });
    source.append("aaa\nbbb\nccc\nddd\neee\nfff\n");
    harness.flush();

    const counts: number[] = [revealedLineCount(harness.screenRows())];
    for (const step of [50, 50, 50, 200, 500]) {
      harness.advance(step);
      counts.push(revealedLineCount(harness.screenRows()));
    }
    // Gradual reveal: starts partial, never regresses, ends complete.
    expect(counts[0]).toBeLessThan(6);
    let previous = counts[0] ?? 0;
    for (const current of counts.slice(1)) {
      expect(current).toBeGreaterThanOrEqual(previous);
      previous = current ?? previous;
    }
    expect(counts[counts.length - 1]).toBe(6);
    expect(harness.screenRows().map((row) => row.trimEnd())).toEqual([
      "aaa",
      "bbb",
      "ccc",
      "ddd",
      "eee",
      "fff",
    ]);
  } finally {
    harness.close();
    source.dispose();
  }
});

test(`${SMOOTH} seal completes a partial reveal and then holds stable`, async () => {
  const harness = await AppHarness.open({ width: 30, height: 6 });
  const source = TextStreamSource.create({
    retention: { maxBytes: 64 * 1024, overflow: "drop-oldest" },
  });
  try {
    const port = harness.contentPort();
    const connector = port.connect(
      source,
      TextFunnel.plain().smooth({ tickIntervalMs: 50, minUnitsPerSecond: 1, maxUnitsPerSecond: 100 }),
    );
    connector.activate();
    harness.render({ body: View.content(port) });
    source.append("aaa\nbbb\n");
    harness.flush();
    harness.advance(1_000);
    const complete = harness.screenRows().map((row) => row.trimEnd()).filter(Boolean);
    // The deterministic host clock owns the elapsed interval. A single
    // 1-second advancement therefore consumes the same elapsed time as
    // repeated small advances; it must not rebase the deadline on every call.
    expect(complete).toEqual(["aaa", "bbb"]);
    source.seal();
    harness.flush();
    // Seal completes the reveal: everything appended becomes visible.
    expect(harness.screenRows().map((row) => row.trimEnd()).filter(Boolean)).toEqual([
      "aaa",
      "bbb",
    ]);
    harness.advance(1_000);
    // Post-seal ticks hold the revealed content stable.
    expect(harness.screenRows().map((row) => row.trimEnd()).filter(Boolean)).toEqual([
      "aaa",
      "bbb",
    ]);
  } finally {
    harness.close();
    source.dispose();
  }
});

test(`${SMOOTH} accumulates repeated one-millisecond advances`, async () => {
  const harness = await AppHarness.open({ width: 30, height: 6 });
  const source = TextStreamSource.create({
    retention: { maxBytes: 64 * 1024, overflow: "drop-oldest" },
  });
  try {
    const port = harness.contentPort();
    const connector = port.connect(
      source,
      TextFunnel.plain().smooth({ tickIntervalMs: 50, minUnitsPerSecond: 1, maxUnitsPerSecond: 100 }),
    );
    connector.activate();
    harness.render({ body: View.content(port) });
    source.append("aaa\nbbb\nccc\nddd\neee\nfff\n");
    harness.flush();
    for (let step = 0; step < 1_000; step += 1) harness.advance(1);
    expect(harness.screenRows().map((row) => row.trimEnd())).toEqual([
      "aaa",
      "bbb",
      "ccc",
      "ddd",
      "eee",
      "fff",
    ]);
  } finally {
    harness.close();
    source.dispose();
  }
});
