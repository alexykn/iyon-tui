import { describe, expect, test } from "bun:test";
import { Scene, Tui, View } from "../src/index.ts";

class Rng {
  private value: number;
  constructor(seed: number) {
    this.value = seed >>> 0;
  }
  next(): number {
    this.value = (this.value * 1664525 + 1013904223) >>> 0;
    return this.value;
  }
  pick(size: number): number {
    return this.next() % size;
  }
}

const TEXT = ["", "ascii", "héllo", "🌍", "line one\nline two", "a\0b"] as const;

function randomView(rng: Rng, depth: number, shared?: View): View {
  if (depth === 0) {
    return View.text(TEXT[rng.pick(TEXT.length)]!).noWrap();
  }
  const reused = shared ?? randomView(rng, depth - 1);
  switch (rng.pick(7)) {
    case 0:
      return View.text(TEXT[rng.pick(TEXT.length)]!).noWrap().textAlign(
        (["start", "center", "end"] as const)[rng.pick(3)]!,
      );
    case 1:
      return View.spacer(rng.pick(3));
    case 2:
      return View.vertical([reused, randomView(rng, depth - 1), randomView(rng, depth - 1)]);
    case 3:
      return View.horizontal([reused, randomView(rng, depth - 1)]);
    case 4:
      return reused.container();
    case 5:
      return reused.clampRows(1 + rng.pick(3));
    default:
      return View.vertical([reused, reused]);
  }
}

describe("PERF-12 T14 retained differential", () => {
  test("100 deterministic DAG seeds match a fresh cold render", async () => {
    const retained = await Tui.open({ width: 32, height: 12, headless: true });
    const cold = await Tui.open({ width: 32, height: 12, headless: true });
    try {
      for (let seed = 0; seed < 100; seed++) {
        const rng = new Rng(seed + 1);
        const base = randomView(rng, 3);
        const next = randomView(rng, 3, base);
        await retained.render(new Scene(base));
        await retained.render(new Scene(next));
        await cold.render(new Scene(next));
        expect(retained.screenRows(), `seed ${seed}`).toEqual(cold.screenRows());
      }
    } finally {
      await retained.close();
      await cold.close();
    }
  });
});
