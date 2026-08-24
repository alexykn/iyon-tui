/**
 * PERF-12 T13.1 R9 — production conversion acceptance (handoff §32.1 R9,
 * §24, §38; AMENDMENT-C §16/§30).
 *
 * The production app plugin consumes the retained execution contract through
 * public APIs only: `defineView` chrome components over tracked `State`
 * slices, canonical `tui.render(() => new Scene(...))`, and builder-owned
 * live tool cards. These tests lock:
 *
 *   - scoped invalidation: a state write executes exactly the reading scope
 *     and zero siblings (counter-proven, never output parity alone);
 *   - effort style-state changes stay confined to Composer + Footer;
 *   - structural toggles execute the owning scope and skip unchanged ones;
 *   - repeated exact-root updates still advance spinners/headless time
 *     (§24.3 side-effect preservation);
 *   - progressive tool-card events execute exactly that card's scope.
 */

import { describe, expect, test } from "bun:test";
import { closeFixture, openFixture, send, advance, transcriptLines } from "./public_app_fixtures.ts";

const counters = async () => await import("../src/view.ts");
const appCounters = async () => await import("../src/app.ts");

describe("T13.1 R9 — production scoped invalidation", () => {
  test("no-op dispatch after mount executes zero chrome bodies", async () => {
    const { resetChromeExecutionCounters, chromeExecutionCounters } = await counters();
    const fixture = await openFixture(80, 24);
    try {
      resetChromeExecutionCounters();
      // turnStarted with no active work flips nothing: a clean-scope pass.
      await send(fixture, { type: "turnStarted" });
      expect(chromeExecutionCounters.root).toBe(0);
      expect(chromeExecutionCounters.working).toBe(0);
      expect(chromeExecutionCounters.footer).toBe(0);
      expect(chromeExecutionCounters.composer).toBe(0);
      expect(chromeExecutionCounters.approval).toBe(0);
    } finally {
      await closeFixture(fixture);
    }
  });

  test("footer status change executes Footer only", async () => {
    const { resetChromeExecutionCounters, chromeExecutionCounters } = await counters();
    const fixture = await openFixture(80, 24);
    try {
      resetChromeExecutionCounters();
      await send(fixture, {
        type: "configChanged",
        provider: "mock",
        modelId: "mock-v2",
        reasoningEffort: "medium",
      });
      expect(chromeExecutionCounters.footer).toBe(1);
      expect(chromeExecutionCounters.composer).toBe(0);
      expect(chromeExecutionCounters.working).toBe(0);
      expect(chromeExecutionCounters.approval).toBe(0);
      expect(chromeExecutionCounters.root).toBe(0);
      expect(transcriptLines(fixture.harness).some((line) => line.includes("mock-v2"))).toBe(true);
    } finally {
      await closeFixture(fixture);
    }
  });

  test("effort cycle is confined to Composer + Footer (style-state + text)", async () => {
    const { resetChromeExecutionCounters, chromeExecutionCounters } = await counters();
    const fixture = await openFixture(80, 24);
    try {
      resetChromeExecutionCounters();
      await fixture.app.handleAction({ type: "cycleReasoningEffort" });
      expect(chromeExecutionCounters.composer).toBe(1);
      expect(chromeExecutionCounters.footer).toBe(1);
      expect(chromeExecutionCounters.working).toBe(0);
      expect(chromeExecutionCounters.approval).toBe(0);
      expect(chromeExecutionCounters.root).toBe(0);
      expect(transcriptLines(fixture.harness).some((line) => line.includes("effort: High"))).toBe(true);
    } finally {
      await closeFixture(fixture);
    }
  });

  test("structural toggle: activity visibility flip executes Working, skips siblings", async () => {
    const { resetChromeExecutionCounters, chromeExecutionCounters } = await counters();
    const fixture = await openFixture(80, 24);
    try {
      resetChromeExecutionCounters();
      // toolCallPreparing carries showActivity: spinner row appears.
      await send(fixture, { type: "toolCallPreparing", key: { messageId: 1, contentIndex: 0 }, toolCallId: "call_a", toolName: "ls" });
      expect(chromeExecutionCounters.working).toBe(1);
      expect(chromeExecutionCounters.footer).toBe(0);
      expect(chromeExecutionCounters.composer).toBe(0);

      resetChromeExecutionCounters();
      // turnCancelled clears live work and hides activity again.
      await send(fixture, { type: "turnCancelled" });
      expect(chromeExecutionCounters.working).toBe(1);
      expect(chromeExecutionCounters.footer).toBe(0);
      expect(chromeExecutionCounters.composer).toBe(0);
      expect(chromeExecutionCounters.root).toBe(0);
    } finally {
      await closeFixture(fixture);
    }
  });

  test("§24.3 side effect preserved: no-op updates still advance headless time", async () => {
    const { resetChromeExecutionCounters, chromeExecutionCounters } = await counters();
    const fixture = await openFixture(80, 24);
    try {
      // Make the spinner visible first (showActivity).
      await send(fixture, { type: "toolCallPreparing", key: { messageId: 1, contentIndex: 0 }, toolCallId: "call_a", toolName: "ls" });
      const rowsBefore = [...fixture.harness.screenRows()];
      advance(fixture, 80, 3);
      const rowsAfterTick = [...fixture.harness.screenRows()];
      // The spinner animates on its native channel while the chrome is quiet.
      expect(rowsAfterTick.some((row, index) => row !== rowsBefore[index])).toBe(true);

      // A no-op action must not disturb the animation channel.
      resetChromeExecutionCounters();
      await fixture.app.handleAction({ type: "ctrlC" });
      expect(chromeExecutionCounters.root).toBe(0);
      advance(fixture, 80, 3);
      const rowsLater = [...fixture.harness.screenRows()];
      expect(rowsLater.some((row, index) => row !== rowsAfterTick[index])).toBe(true);
    } finally {
      await closeFixture(fixture);
    }
  });

  test("progressive tool events execute exactly that card's scope", async () => {
    const { toolCardExecutionCounters } = await appCounters();
    const fixture = await openFixture(80, 24);
    try {
      await send(fixture, { type: "toolCallPreparing", key: { messageId: 1, contentIndex: 0 }, toolCallId: "call_a", toolName: "ls" });
      await send(fixture, { type: "toolCallArguments", key: { messageId: 1, contentIndex: 0 }, delta: "{\"path\":\".\"}" });
      await send(fixture, { type: "toolCallPrepared", key: { messageId: 1, contentIndex: 0 }, toolCallId: "call_a", toolName: "ls", arguments: { path: "." } });
      await send(fixture, { type: "toolCallStarted", toolCallId: "call_a", toolName: "ls", arguments: { path: "." } });

      const callsA = toolCardExecutionCounters.get("1:0") ?? 0;
      // Each progressive event re-runs exactly this card's body.
      expect(callsA).toBeGreaterThan(0);

      // The bullet line reflects lifecycle through the scoped component
      // (asserted while card 1 alone is mounted and on-screen).
      expect(transcriptLines(fixture.harness).some((line) => line.includes("ls . — running"))).toBe(true);

      // A second card mounts and streams; the first card's body must not
      // run again while it happens.
      await send(fixture, { type: "toolCallPreparing", key: { messageId: 2, contentIndex: 0 }, toolCallId: "call_b", toolName: "read" });
      expect(toolCardExecutionCounters.get("2:0") ?? 0).toBe(1);
      await send(fixture, { type: "toolCallArguments", key: { messageId: 2, contentIndex: 0 }, delta: "{}", toolName: "read" });
      expect(toolCardExecutionCounters.get("2:0") ?? 0).toBe(2);
      expect(toolCardExecutionCounters.get("1:0") ?? 0).toBe(callsA);
    } finally {
      await closeFixture(fixture);
    }
  });
});
