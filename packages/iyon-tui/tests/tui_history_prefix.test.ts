import { expect, test } from "bun:test";

import { View } from "../src/index.ts";
import { AppHarness } from "../src/testing/index.ts";

const HISTORY_PREFIX = "PRE-V5-L1 stable History-prefix characterization";

function screenText(harness: { screenRows(): readonly string[] }): string[] {
  return harness.screenRows().map((row) => row.trimEnd());
}

/// History units render above the body with the body pinned last, so the
/// History sequence is every non-empty row except the final body row.
/// Native scrollback transfer only fires once units scroll off-screen.
function historySequence(rows: readonly string[]): string[] {
  const nonEmpty = rows.filter((row) => row.length > 0);
  return nonEmpty.slice(0, -1);
}

test(`${HISTORY_PREFIX} appended units extend output without moving the settled prefix`, async () => {
  const harness = await AppHarness.open({ width: 30, height: 12 });
  try {
    const history = harness.createHistory();
    history.push(View.text("prefix one"));
    history.push(View.text("prefix two"));
    history.push(View.text("prefix three"));
    harness.render({ body: View.text("live body"), history });
    const screen = screenText(harness);
    expect(historySequence(screen)).toEqual(["prefix one", "prefix two", "prefix three"]);
    expect(screen.at(-1)).toBe("live body");

    // Re-render with no changes: projection must be idempotent.
    harness.render({ body: View.text("live body"), history });
    expect(screenText(harness)).toEqual(screen);

    // Append: the settled sequence must survive verbatim as a prefix.
    history.push(View.text("prefix four"));
    harness.render({ body: View.text("live body"), history });
    const extended = screenText(harness);
    expect(historySequence(extended)).toEqual([
      "prefix one",
      "prefix two",
      "prefix three",
      "prefix four",
    ]);
    expect(extended.at(-1)).toBe("live body");
  } finally {
    harness.close();
  }
});

test(`${HISTORY_PREFIX} freezing a live tail unit swaps only the tail`, async () => {
  const harness = await AppHarness.open({ width: 30, height: 12 });
  try {
    const history = harness.createHistory();
    history.push(View.text("settled one"));
    const slot = harness.createViewSlot(View.text("live tail"));
    const live = history.push(slot.view());
    harness.render({ body: View.text("body"), history });
    const before = screenText(harness);
    expect(historySequence(before)).toEqual(["settled one", "live tail"]);

    history.freeze(live, View.text("final tail"));
    harness.render({ body: View.text("body"), history });
    const after = screenText(harness);
    expect(historySequence(after)).toEqual(["settled one", "final tail"]);
    expect(after.at(-1)).toBe("body");
    slot.dispose();
  } finally {
    harness.close();
  }
});
