/**
 * PERF-12 T13.1 R9 — scoped-invalidation acceptance tests (handoff §32.1 R9,
 * AMENDMENT-C §24, handoff §32.2.6).
 *
 * An external consumer using ONLY public APIs (defineView/state/View.key/
 * View/Tui) receives retained execution automatically with zero setup:
 *
 *   - a state write executes exactly the reading scope (§31.1 gate);
 *   - keyed reorder preserves identity AND skips bodies;
 *   - direct/builder ownership transitions never ghost;
 *   - 10k stream appends cause zero scope executions (isolation gate
 *     §32.2.6).
 */

import { describe, expect, test } from "bun:test";
import { state } from "@iyon/runtime/tui";
import { TextStream, View } from "@iyon/runtime/tui";
import {
  buildScopedConsumer,
  openConsumerSession,
  type ConsumerState,
} from "../src/consumer.ts";

function baseState(overrides: Partial<ConsumerState> = {}): ConsumerState {
  return {
    title: "fixture",
    status: "idle",
    items: ["alpha", "beta"],
    showHint: true,
    ...overrides,
  };
}

async function drain(): Promise<void> {
  // Two microtask hops: scheduleFlush queues M1; await queues C after M1;
  // M1 runs flush; C resumes seeing post-flush state.
  await Promise.resolve();
  await Promise.resolve();
}

describe("T13.1 R9 — scoped invalidation acceptance", () => {
  test("§31.1 frontier gate: write B ⇒ App=0 A=0 B=1 C=0", async () => {
    const session = await openConsumerSession();
    try {
      const consumer = buildScopedConsumer(session.tui);

      // Initial canonical render: every scope evaluates exactly once.
      consumer.renderApp();
      await drain();

      expect(consumer.headerExecutions()).toBe(1);
      expect(consumer.cardExecutions("a")).toBe(1);
      expect(consumer.cardExecutions("b")).toBe(1);

      // Local state change: only the reading scope re-executes.
      consumer.status.set("running");
      await drain();

      expect(consumer.headerExecutions()).toBe(2);
      expect(consumer.cardExecutions("a")).toBe(1);
      expect(consumer.cardExecutions("b")).toBe(1);

      // The new header text is visible without any explicit re-render.
      expect(session.tui.screenRows().join("\n")).toContain(
        "header [running]",
      );
    } finally {
      session.close();
    }
  });

  test("keyed reorder preserves identity and skips bodies", async () => {
    const session = await openConsumerSession();
    try {
      const consumer = buildScopedConsumer(session.tui);
      consumer.renderApp();
      await drain();

      const baseA = consumer.cardExecutions("a");
      const baseB = consumer.cardExecutions("b");
      expect(baseA).toBe(1);
      expect(baseB).toBe(1);

      // Reorder: per-instance props are shallow-equal, so bodies must NOT
      // re-execute (identity follows keys).
      consumer.items.set([...consumer.items.value].reverse());
      consumer.renderApp();
      await drain();

      expect(consumer.cardExecutions("a")).toBe(baseA);
      expect(consumer.cardExecutions("b")).toBe(baseB);

      // Content change on one key executes exactly that card once.
      consumer.items.set([
        { id: "b", label: "beta-2" },
        { id: "a", label: "alpha" },
      ]);
      consumer.renderApp();
      await drain();

      expect(consumer.cardExecutions("b")).toBe(baseB + 1);
      expect(consumer.cardExecutions("a")).toBe(baseA);
    } finally {
      session.close();
    }
  });

  test("ownership modes never ghost: builder → direct → stale builder is inert", async () => {
    const session = await openConsumerSession();
    try {
      const slot = session.listSlot;
      const label = state("v1");

      // Render chrome so the slot's component ref is mounted in a scene.
      session.render(baseState());
      expect(session.tui.screenRows().join("\n")).toContain("fixture");

      // Builder mode takes ownership; paints immediately.
      slot.setView(() =>
        View.text(label.value),
      );
      await drain();
      expect(session.tui.screenRows().join("\n")).toContain("v1");

      // Tracked write drives it WITHOUT another setView call.
      label.set("v2");
      await drain();
      expect(session.tui.screenRows().join("\n")).toContain("v2");

      // DIRECT takes ownership: builder disposed after install succeeds.
      slot.setView(View.text("fixed"));
      await drain();
      expect(session.tui.screenRows().join("\n")).toContain("fixed");

      // Stale builder must NOT ghost-overwrite the direct value.
      label.set("v3");
      await drain();
      const screen = session.tui.screenRows().join("\n");
      expect(screen).not.toContain("v3");
      expect(screen).toContain("fixed");
    } finally {
      session.close();
    }
  });

  test("isolation gate: 10k stream appends ⇒ zero scope executions", async () => {
    const session = await openConsumerSession();
    try {
      const consumer = buildScopedConsumer(session.tui);
      consumer.renderApp();
      await drain();
      const baselineHeader = consumer.headerExecutions();
      const baselineA = consumer.cardExecutions("a");

      const stream = new TextStream();
      const history = session.history;
      history.pushStream(stream as never);

      for (let index = 0; index < 10_000; index += 1) {
        stream.append(`line ${index}\n`);
      }
      await drain();

      // Zero scope executions: no body ran because of stream appends.
      expect(consumer.headerExecutions()).toBe(baselineHeader);
      expect(consumer.cardExecutions("a")).toBe(baselineA);
      expect(consumer.cardExecutions("b")).toBe(1);

      stream.seal();
      history.sealStream(stream as never);
    } finally {
      session.close();
    }
  });
});
