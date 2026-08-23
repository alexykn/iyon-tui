/**
 * PERF-12 T13.1 §13.3 external-consumer fixture — acceptance tests (skeleton).
 *
 * Step 1 scope: prove the consumer-shaped sources behave correctly through
 * the public API alone, BEFORE the composition runtime exists. Later T13.1
 * steps extend THIS file with automatic-composition assertions (exact View
 * reuse, changed-frontier counters, keyed reorder) without changing
 * `consumer.ts` and without adding any setup to this package.
 *
 * Drives (§13.3): exact no-op, single text change, conditional branch toggle,
 * ViewSlot recurring update, ScrollPane recurring update. The dynamic keyed
 * reorder case activates together with the public `View.key` API.
 */

import { describe, expect, test } from "bun:test";
import { View } from "@iyon/runtime/tui";
import {
  buildConsumerBody,
  buildItemRows,
  consumerFooterText,
  openConsumerSession,
  type ConsumerState,
} from "../src/consumer.ts";

function state(overrides: Partial<ConsumerState> = {}): ConsumerState {
  return { title: "fixture", status: "idle", items: ["alpha", "beta"], showHint: true, ...overrides };
}

async function screenOf(tui: Awaited<ReturnType<typeof openConsumerSession>>["tui"]): Promise<string> {
  return tui.screenRows().map((row) => row.replace(/\s+$/, "")).join("\n");
}

describe("T13.1 external-consumer fixture skeleton", () => {
  test("public surface only: fixture imports resolve and chrome renders", async () => {
    const session = await openConsumerSession();
    try {
      session.render(state());
      const screen = await screenOf(session.tui);
      expect(screen).toContain("fixture");
      expect(screen).toContain(consumerFooterText(state()));
    } finally {
      session.close();
    }
  });

  test("exact semantic no-op: re-rendering an identical state keeps the screen stable", async () => {
    const session = await openConsumerSession();
    try {
      const first = state();
      session.listSlot.setView(buildItemRows(first.items));
      session.render(first);
      // Same STATE values, freshly constructed body — ordinary app behavior.
      session.render(state());
      session.render(state({ items: [...first.items] }));
      const screen = await screenOf(session.tui);
      expect(screen).toContain("- alpha");
      expect(screen).toContain("- beta");
    } finally {
      session.close();
    }
  });

  test("single text change: footer text update is visible", async () => {
    const session = await openConsumerSession();
    try {
      session.render(state({ status: "idle" }));
      session.render(state({ status: "streaming" }));
      const screen = await screenOf(session.tui);
      expect(screen).toContain(consumerFooterText(state({ status: "streaming" })));
      expect(screen).not.toContain(consumerFooterText(state({ status: "idle" })));
    } finally {
      session.close();
    }
  });

  test("conditional branch toggle: hint row appears and disappears cleanly", async () => {
    const session = await openConsumerSession();
    try {
      session.render(state({ showHint: true }));
      expect(await screenOf(session.tui)).toContain("type a message");
      session.render(state({ showHint: false }));
      expect(await screenOf(session.tui)).not.toContain("type a message");
      session.render(state({ showHint: true }));
      expect(await screenOf(session.tui)).toContain("type a message");
    } finally {
      session.close();
    }
  });

  test("ViewSlot recurring update: latest content wins every time", async () => {
    const session = await openConsumerSession();
    try {
      session.render(state());
      for (let round = 0; round < 5; round += 1) {
        session.listSlot.setView(buildItemRows([`row-${round}-a`, `row-${round}-b`]));
      }
      const screen = await screenOf(session.tui);
      expect(screen).toContain("row-4-a");
      expect(screen).toContain("row-4-b");
      expect(screen).not.toContain("row-0-a");
    } finally {
      session.close();
    }
  });

  test("ScrollPane recurring update: content lands and stays after followEnd", async () => {
    const session = await openConsumerSession();
    try {
      session.render(state());
      for (let round = 0; round < 5; round += 1) {
        session.pane.setContent(View.vertical([View.text(`pane line ${round}`).fillWidth()]).fillWidth());
        session.pane.followEnd();
      }
      expect(await screenOf(session.tui)).toContain("pane line 4");
    } finally {
      session.close();
    }
  });
});
